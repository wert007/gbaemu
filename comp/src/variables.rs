use std::{collections::HashMap, ops::Index};

use crate::{HasLocation, Location, StringId, typing::TypeId};

#[derive(Debug)]
pub struct VariableDeclaration {
    pub location: Location,
    pub id: VariableId,
    pub namespaces: Vec<StringId>,
    pub name: StringId,
    pub type_: TypeId,
    pub scope: ScopeId,
    pub can_be_overshadowed: bool,
}

impl HasLocation for VariableDeclaration {
    fn location(&self) -> Location {
        self.location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(usize);
impl ScopeId {
    pub fn as_raw(&self) -> usize {
        self.0
    }
    fn increase(&mut self) {
        self.0 += 1;
    }
}
#[derive(Debug)]
pub struct Scope {
    pub(crate) id: ScopeId,
    pub(crate) parents: Vec<ScopeId>,
}

impl Scope {
    pub fn global() -> Self {
        Scope {
            id: ScopeId(0),
            parents: Vec::new(),
        }
    }

    fn create_child(&self, next_scope_id: &mut ScopeId) -> Scope {
        let mut parents = self.parents.clone();
        parents.push(self.id);
        next_scope_id.increase();
        let id = *next_scope_id;
        Scope { id, parents }
    }
}

#[derive(Debug)]
pub struct Variables {
    pub(crate) variables: HashMap<ScopeId, Vec<VariableDeclaration>>,
    pub(crate) all_scopes: Vec<Scope>,
    pub(crate) active_scope: ScopeId,
    next_variable_id: usize,
}

impl Variables {
    pub fn new() -> Self {
        let mut variables = HashMap::new();
        let active_scope = ScopeId(0);
        let global = Scope::global();
        variables.insert(active_scope, Vec::new());
        let result = Self {
            variables,
            all_scopes: vec![global],
            active_scope,
            next_variable_id: 0,
        };
        result
    }

    pub fn register(
        &mut self,
        location: Location,
        namespaces: &[StringId],
        name: StringId,
        type_: TypeId,
        can_be_overshadowed: bool,
    ) -> Option<VariableId> {
        let current_scope = self.active_scope;
        if self.variables[&current_scope]
            .iter()
            .any(|v| !v.can_be_overshadowed && v.name == name && v.namespaces == namespaces)
        {
            None
        } else {
            let id = VariableId(self.next_variable_id, name);
            self.next_variable_id += 1;
            self.variables
                .get_mut(&current_scope)
                .unwrap()
                .push(VariableDeclaration {
                    scope: current_scope,
                    location,
                    namespaces: namespaces.to_vec(),
                    id,
                    name,
                    type_,
                    can_be_overshadowed,
                });
            Some(id)
        }
    }

    pub fn find_by_name(&self, name: StringId) -> Option<&VariableDeclaration> {
        for scope in self.visible_scopes() {
            if let Some(it) = self.variables[&scope].iter().find(|v| v.name == name) {
                return Some(it);
            }
        }
        None
    }

    pub fn start_scope(&mut self) {
        let mut scope = ScopeId(self.all_scopes.len() - 1);
        let child = self.active_scope().create_child(&mut scope);
        self.active_scope = scope;
        self.variables.insert(scope, Vec::new());
        self.all_scopes.push(child);
    }

    pub fn end_scope(&mut self) {
        self.active_scope = self.visible_scopes().skip(1).next().unwrap();
    }

    fn visible_scopes(&self) -> ScopeIter<'_> {
        ScopeIter {
            current: Some(self.active_scope),
            all_scopes: &self.all_scopes,
        }
    }

    fn active_scope(&self) -> &Scope {
        &self.all_scopes[self.active_scope.0]
    }

    pub(crate) fn type_of(&self, id: VariableId) -> TypeId {
        self[id].type_
    }
}

impl Index<VariableId> for Variables {
    type Output = VariableDeclaration;

    fn index(&self, index: VariableId) -> &Self::Output {
        for value in self.variables.values() {
            if let Ok(index) = value.binary_search_by_key(&index, |v| v.id) {
                return &value[index];
            }
        }
        unreachable!("All variable ids should be used!")
    }
}

struct ScopeIter<'a> {
    current: Option<ScopeId>,
    all_scopes: &'a [Scope],
}

impl<'a> Iterator for ScopeIter<'a> {
    type Item = ScopeId;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.current.take()?;
        self.current = self.all_scopes[result.0].parents.last().copied();
        Some(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableId(usize, pub(crate) StringId);

impl VariableId {
    pub fn as_raw(&self) -> usize {
        self.0
    }
}

impl PartialOrd for VariableId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for VariableId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
