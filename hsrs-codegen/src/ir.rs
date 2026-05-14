pub struct ParsedFile {
    pub enums: Vec<FfiEnum>,
    pub modules: Vec<FfiModule>,
    pub value_types: Vec<FfiValueType>,
}

pub struct FfiEnum {
    pub name: String,
    pub variants: Vec<String>,
    pub has_eq: bool,
    pub has_show: bool,
    pub has_ord: bool,
    pub docs: Vec<String>,
}

pub struct FfiModule {
    pub name: String,
    pub struct_name: String,
    pub functions: Vec<FfiFunction>,
    pub docs: Vec<String>,
}

pub struct FfiValueType {
    pub name: String,
    pub fields: Vec<FfiField>,
    pub has_eq: bool,
    pub has_show: bool,
    pub has_ord: bool,
    pub docs: Vec<String>,
}

pub struct FfiField {
    pub name: String,
    pub ty: FfiType,
}

pub struct FfiFunction {
    pub rust_name: String,
    pub c_name: String,
    pub kind: FfiFunctionKind,
    pub safety: FfiSafety,
    pub params: Vec<FfiParam>,
    pub return_type: Option<FfiType>,
    pub docs: Vec<String>,
    pub borsh_return: bool,
    pub borsh_params: Vec<String>,
}

pub enum FfiFunctionKind {
    Constructor,
    MutMethod,
    RefMethod,
    Destructor,
}

pub enum FfiSafety {
    Safe,
    Unsafe,
    Interruptible,
}

pub struct FfiParam {
    pub name: String,
    pub ty: FfiType,
}

pub enum FfiType {
    Int(u8),
    Uint(u8),
    Bool,
    Usize,
    Isize,
    Enum(String),
    Unit,
    ValueType(String),
    Result(Box<FfiType>, Box<FfiType>),
    Option(Box<FfiType>),
}
