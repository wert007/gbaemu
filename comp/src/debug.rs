use std::fmt::Display;

use crate::{
    StringInterner,
    bind::{ScopeId, Variables},
    typing::{TypeId, Types},
};

struct TypeToString<'a, 'b> {
    types: &'a Types,
    strings: &'b StringInterner,
    id: TypeId,
}

impl Display for TypeToString<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.types.fmt_type(self.id, f, self.strings)
    }
}

impl Variables {
    pub fn dump(&self, strings: &StringInterner, types: &Types) {
        for scope in &self.all_scopes {
            eprintln!(
                " -- Scope {} ({} Variables) -- ",
                scope.id.as_raw(),
                &self.variables[&scope.id].len()
            );
            for variable in &self.variables[&scope.id] {
                let mut name = String::new();
                for namespace in &variable.namespaces {
                    name.push_str(&strings[*namespace]);
                    name.push_str("::");
                }
                name.push_str(&strings[variable.name]);

                eprintln!(
                    "    {name}: {}",
                    TypeToString {
                        types,
                        id: variable.type_,
                        strings
                    }
                );
            }
        }
    }
}
