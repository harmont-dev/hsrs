pub struct ParsedFile {
    pub enums: Vec<FfiEnum>,
    pub modules: Vec<FfiModule>,
}

pub struct FfiEnum {
    pub name: String,
    pub variants: Vec<String>,
    pub has_eq: bool,
    pub has_show: bool,
    pub has_ord: bool,
}

pub struct FfiModule {
    pub name: String,
    pub struct_name: String,
    pub functions: Vec<FfiFunction>,
}

pub struct FfiFunction {
    pub rust_name: String,
    pub c_name: String,
    pub kind: FfiFunctionKind,
    pub params: Vec<FfiParam>,
    pub return_type: Option<FfiType>,
}

pub enum FfiFunctionKind {
    Constructor,
    MutMethod,
    RefMethod,
    Destructor,
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
}
