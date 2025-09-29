use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(xdr);
pub mod ast;
use ast::*;

#[derive(Debug)]
struct Token {
    start: u32,
    end: u32,
    val: String,
}

#[derive(Debug)]
struct Backend {
    client: Client,
    // Used to find the identifier at a given location, note the vector must be sorted
    // file -> line -> list of identifiers
    identifiers: Mutex<HashMap<PathBuf, HashMap<u32, Vec<Token>>>>,
    // TODO: these probably should also take a file parameter so they can be updated
    // Used to find references to a given identifier, the vector is unsorted
    referenced_locs: Mutex<HashMap<String, Vec<Location>>>,
    // Used to find where identifiers are defined
    defn_locs: Mutex<HashMap<String, Location>>,
}

fn make_error(code: i64, message: &'static str) -> Error {
    Error {
        code: tower_lsp::jsonrpc::ErrorCode::ServerError(code),
        message: std::borrow::Cow::Borrowed(message),
        data: None,
    }
}

fn get_xdr_files(dir: &PathBuf, cb: &mut dyn FnMut(&PathBuf)) {
    if dir.is_dir()
        && let Ok(entries) = fs::read_dir(dir)
    {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    get_xdr_files(&path, cb);
                } else if path.extension().is_some_and(|ext| ext == "x") {
                    cb(&path);
                }
            }
        }
    }
}

fn visit_identifiers(spec: &Specification, cb: &mut dyn FnMut(&Identifier, bool)) {
    for defn in &spec.defns {
        visit_identifiers_defn(defn, cb);
    }
}

fn visit_identifiers_defn(defn: &Definition, cb: &mut dyn FnMut(&Identifier, bool)) {
    match defn {
        Definition::Constant { id, .. } => {
            cb(id, true);
        }
        Definition::TypeDef(decl) => {
            visit_identifiers_decl(decl, true, cb);
        }
        Definition::Enum { id, body } => {
            cb(id, true);
            visit_identifiers_enum(body, cb);
        }
        Definition::Struct { id, body } => {
            cb(id, true);
            visit_identifiers_struct(body, cb);
        }
        Definition::Union { id, body } => {
            cb(id, true);
            visit_identifiers_union(body, cb);
        }
    }
}

fn visit_identifiers_decl(
    decl: &Declaration,
    in_defn: bool,
    cb: &mut dyn FnMut(&Identifier, bool),
) {
    match decl {
        Declaration::Normal { spec, id } | Declaration::Optional { spec, id } => {
            visit_identifiers_type(spec, cb);
            cb(id, in_defn);
        }
        Declaration::FixedArr { spec, id, size } => {
            visit_identifiers_type(spec, cb);
            cb(id, in_defn);
            visit_identifiers_val(size, cb);
        }
        Declaration::VarArr { spec, id, size } => {
            visit_identifiers_type(spec, cb);
            cb(id, in_defn);
            if let Some(size) = size {
                visit_identifiers_val(size, cb)
            }
        }
        Declaration::FixedOpaque { id, size } => {
            cb(id, in_defn);
            visit_identifiers_val(size, cb)
        }
        Declaration::VarOpaque { id, size } | Declaration::String { id, size } => {
            cb(id, in_defn);
            if let Some(size) = size {
                visit_identifiers_val(size, cb)
            }
        }
        Declaration::VOID => {}
    }
}

fn visit_identifiers_enum(body: &EnumBody, cb: &mut dyn FnMut(&Identifier, bool)) {
    for EnumAssign { id, val } in &body.body {
        cb(id, true);
        visit_identifiers_val(val, cb);
    }
}

fn visit_identifiers_struct(body: &StructBody, cb: &mut dyn FnMut(&Identifier, bool)) {
    for decl in &body.body {
        visit_identifiers_decl(decl, false, cb);
    }
}

fn visit_identifiers_union(body: &UnionBody, cb: &mut dyn FnMut(&Identifier, bool)) {
    visit_identifiers_decl(&body.discriminant, false, cb);
    for CaseSpec { values, decl } in &body.cases {
        for val in values {
            visit_identifiers_val(val, cb);
        }
        visit_identifiers_decl(decl, false, cb);
    }
    if let Some(decl) = &body.default {
        visit_identifiers_decl(decl, false, cb);
    }
}

fn visit_identifiers_val(val: &Value, cb: &mut dyn FnMut(&Identifier, bool)) {
    if let Value::Id(id) = val {
        cb(id, false);
    }
}

fn visit_identifiers_type(body: &TypeSpecifier, cb: &mut dyn FnMut(&Identifier, bool)) {
    match body {
        TypeSpecifier::BuiltIn(_) => {}
        TypeSpecifier::Enum(body) => visit_identifiers_enum(body, cb),
        TypeSpecifier::Struct(body) => visit_identifiers_struct(body, cb),
        TypeSpecifier::Union(body) => visit_identifiers_union(body, cb),
        TypeSpecifier::Ident(id) => cb(id, false),
    }
}

// TODO: probably want to actually pass back the errors
// TODO: the return value is meaningless: it's just there so we can use the ? for early returns
fn parse_file(
    path: &PathBuf,
    identifiers: &mut HashMap<u32, Vec<Token>>,
    ref_locs: &mut HashMap<String, Vec<Location>>,
    defn_locs: &mut HashMap<String, Location>,
) -> Option<()> {
    let uri: Url = Url::from_file_path(path).ok()?;
    let file = fs::read_to_string(path).ok()?;
    let spec = xdr::SpecificationParser::new().parse(&file).ok()?;

    // Collect line numbers
    let line_locs: Vec<usize> = file
        .char_indices()
        .filter(|(i, c)| *i == 0 || *c == '\n')
        .map(|(i, _)| if i == 0 { 0 } else { i + 1 })
        .collect();

    visit_identifiers(&spec, &mut |id, is_defn| {
        let start = id.start;
        let line = line_locs.partition_point(|x| x <= &start) - 1;
        let scol = id.start - line_locs[line];
        let ecol = id.end - line_locs[line];
        let loc = Location {
            uri: uri.clone(),
            range: Range {
                start: Position {
                    line: line as u32,
                    character: scol as u32,
                },
                end: Position {
                    line: line as u32,
                    character: ecol as u32,
                },
            },
        };

        identifiers.entry(line as u32).or_default().push(Token {
            start: scol as u32,
            end: ecol as u32,
            val: id.id.clone(),
        });
        if is_defn {
            defn_locs.insert(id.id.clone(), loc);
        } else {
            // Note: this way we can handle when the client requests references not including
            // definition location
            ref_locs.entry(id.id.clone()).or_default().push(loc);
        }
    });
    let keys: Vec<_> = identifiers.keys().map(|k| *k).collect();
    for key in keys {
        if let Some(vec) = identifiers.get_mut(&key) {
            vec.sort_by_key(|t| t.start);
        }
    }
    None
}

impl Backend {
    fn new(client: Client) -> Self {
        Backend {
            client: client,
            identifiers: Mutex::new(HashMap::new()),
            referenced_locs: Mutex::new(HashMap::new()),
            defn_locs: Mutex::new(HashMap::new()),
        }
    }

    async fn get_ident_at(&self, path: &PathBuf, pos: Position) -> Option<String> {
        let Position {
            line,
            character: ch,
        } = pos;
        self.identifiers
            .lock()
            .await
            .get(path)
            .and_then(|m| m.get(&line))
            .and_then(|idents| {
                let index = idents.partition_point(|i| i.start <= ch);
                if index == 0 {
                    None
                } else {
                    idents.get(index - 1)
                }
            })
            .and_then(|token| {
                if token.start <= ch && ch <= token.end {
                    Some(token.val.clone())
                } else {
                    None
                }
            })
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let uri = match params.root_uri {
            Some(uri) => uri,
            None => {
                return Err(make_error(
                    0,
                    "This language server requires root_uri to be set",
                ));
            }
        };
        let uri = match uri.to_file_path() {
            Ok(uri) => uri,
            Err(_) => {
                return Err(make_error(
                    0,
                    "root_uri doesn't seem to be a valid filepath",
                ));
            }
        };
        if !uri.is_dir() {
            return Err(make_error(0, "root_uri doesn't name a directory"));
        }
        let mut paths: Vec<PathBuf> = vec![];
        get_xdr_files(&uri, &mut |path| paths.push(path.to_path_buf()));
        {
            let mut identifiers = self.identifiers.lock().await;
            let mut refs = self.referenced_locs.lock().await;
            let mut defns = self.defn_locs.lock().await;
            for path in &paths {
                parse_file(
                    path,
                    identifiers.entry(path.to_path_buf()).or_default(),
                    &mut refs,
                    &mut defns,
                );
            }
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        if let Ok(path) = params
            .text_document_position_params
            .text_document
            .uri
            .to_file_path()
        {
            match self
                .get_ident_at(&path, params.text_document_position_params.position)
                .await
            {
                None => Ok(None),
                Some(ident) => match self.defn_locs.lock().await.get(&ident) {
                    Some(loc) => Ok(Some(GotoDefinitionResponse::Scalar(loc.clone()))),
                    None => Ok(None),
                },
            }
        } else {
            Err(make_error(0, "Could not open file"))
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        if let Ok(path) = params
            .text_document_position
            .text_document
            .uri
            .to_file_path()
        {
            match self
                .get_ident_at(&path, params.text_document_position.position)
                .await
            {
                None => Ok(None),
                Some(ident) => match self.referenced_locs.lock().await.get(&ident) {
                    Some(locs) => Ok(Some({
                        let mut locs = locs.clone();
                        if params.context.include_declaration {
                            if let Some(decl) = self.defn_locs.lock().await.get(&ident) {
                                locs.push(decl.clone());
                            }
                        }
                        locs
                    })),
                    None => Ok(None),
                },
            }
        } else {
            Err(make_error(0, "Could not open file"))
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
