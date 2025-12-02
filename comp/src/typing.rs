use std::ops::Index;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeId(usize);

impl TypeId {
    pub const ERROR: TypeId = TypeId(0);
    pub const UNKNOWN: TypeId = TypeId(1);
    pub const VOID: TypeId = TypeId(2);
    pub const INTEGER: TypeId = TypeId(3);
    pub const BOOL: TypeId = TypeId(4);
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Error,
    Unknown,
    Void,
    Integer,
    Bool,
    Array(TypeId, usize),
}

#[derive(Debug)]
pub struct Types {
    types: Vec<Type>,
}

impl Types {
    pub fn new() -> Self {
        let result = Self {
            types: vec![
                Type::Error,
                Type::Unknown,
                Type::Void,
                Type::Integer,
                Type::Bool,
            ],
        };
        assert_eq!(result[TypeId::ERROR], Type::Error);
        assert_eq!(result[TypeId::UNKNOWN], Type::Unknown);
        assert_eq!(result[TypeId::VOID], Type::Void);
        assert_eq!(result[TypeId::INTEGER], Type::Integer);
        result
    }

    pub fn register(&mut self, type_: Type) -> TypeId {
        if let Some(position) = self.types.iter().position(|t| t == &type_) {
            TypeId(position)
        } else {
            let index = self.types.len();
            self.types.push(type_);
            TypeId(index)
        }
    }
}

impl Index<TypeId> for Types {
    type Output = Type;

    fn index(&self, index: TypeId) -> &Self::Output {
        &self.types[index.0]
    }
}
