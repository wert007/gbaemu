use std::{collections::HashMap, ops::Index};

use crate::{
    BoundId, StringId,
    typing::{FunctionType, TypeId, Types},
    variables::VariableId,
};

pub struct TraitImplementors {
    trait_to_types: HashMap<TraitId, Vec<TypeId>>,
    type_to_trait: HashMap<TypeId, Vec<TraitId>>,
}

impl TraitImplementors {
    pub fn new() -> Self {
        Self {
            trait_to_types: HashMap::new(),
            type_to_trait: HashMap::new(),
        }
    }

    pub(crate) fn find_traits(&self, base_type: TypeId) -> &[TraitId] {
        self.type_to_trait
            .get(&base_type)
            .map(|t| t.as_slice())
            .unwrap_or_default()
    }

    pub(crate) fn register(&mut self, type_: TypeId, trait_: TraitId) {
        self.type_to_trait.entry(type_).or_default().push(trait_);
        self.trait_to_types.entry(trait_).or_default().push(type_);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraitId(usize);

pub struct Traits {
    traits: Vec<Trait>,
}

impl Traits {
    pub fn new() -> Self {
        Self { traits: Vec::new() }
    }

    pub fn register(&mut self, trait_: Trait) -> TraitId {
        if let Some(index) = self.traits.iter().position(|t| t.name == trait_.name) {
            TraitId(index)
        } else {
            let index = self.traits.len();
            self.traits.push(trait_);
            TraitId(index)
        }
    }
}

impl Index<TraitId> for Traits {
    type Output = Trait;

    fn index(&self, index: TraitId) -> &Self::Output {
        &self.traits[index.0]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TraitFunction {
    pub type_: TypeId,
    pub name: VariableId,
    // TODO: How will this look outside const evaluation?
    pub implementation: BoundId,
}

pub struct Trait {
    pub name: VariableId,
    pub functions: Vec<TraitFunction>,
}

impl Trait {
    pub fn find_function_by_name(&self, name: StringId) -> Option<TraitFunction> {
        self.functions.iter().find(|f| f.name.1 == name).copied()
    }
}
