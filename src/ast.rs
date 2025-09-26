#[derive(Debug)]
pub enum Declaration {
    Normal {
        spec: TypeSpecifier,
        id: Identifier,
    },
    FixedArr {
        spec: TypeSpecifier,
        id: Identifier,
        size: Value,
    },
    VarArr {
        spec: TypeSpecifier,
        id: Identifier,
        size: Option<Value>,
    },
    FixedOpaque {
        id: Identifier,
        size: Value,
    },
    VarOpaque {
        id: Identifier,
        size: Option<Value>,
    },
    String {
        id: Identifier,
        size: Option<Value>,
    },
    Optional {
        spec: TypeSpecifier,
        id: Identifier,
    },
    VOID,
}

#[derive(Debug)]
pub enum Value {
    Id(Identifier),
    Const(String),
}

#[derive(Debug)]
pub enum TypeSpecifier {
    BuiltIn(String),
    Enum(EnumBody),
    Struct(StructBody),
    Union(UnionBody),
    Ident(Identifier),
}

#[derive(Debug)]
pub struct EnumAssign {
    pub id: Identifier,
    pub val: Value,
}

#[derive(Debug)]
pub struct EnumBody {
    pub body: Vec<EnumAssign>,
}

#[derive(Debug)]
pub struct StructBody {
    pub body: Vec<Declaration>,
}

#[derive(Debug)]
pub struct UnionBody {
    // Boxes because of recursion
    pub discriminant: Box<Declaration>,
    pub cases: Vec<CaseSpec>,
    pub default: Option<Box<Declaration>>,
}

#[derive(Debug)]
pub struct CaseSpec {
    pub values: Vec<Value>,
    pub decl: Declaration,
}

#[derive(Debug)]
pub enum Definition {
    Constant { id: Identifier, val: String },
    TypeDef(Declaration),
    Enum { id: Identifier, body: EnumBody },
    Struct { id: Identifier, body: StructBody },
    Union { id: Identifier, body: UnionBody },
}

#[derive(Debug)]
pub struct Specification {
    pub defns: Vec<Definition>,
}

#[derive(Debug)]
pub struct Identifier {
    pub id: String,
    pub start: usize,
    pub end: usize,
}
