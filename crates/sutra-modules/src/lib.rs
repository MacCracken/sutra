//! sutra-modules — Built-in module implementations for infrastructure orchestration.

use sutra_core::{Executor, NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub mod argonaut;
pub mod ark;
pub mod file;
pub mod shell;
pub mod user;
pub mod verify;

/// Enum dispatch for all built-in modules (avoids dyn + async incompatibility).
pub enum Module {
    Ark(ark::ArkModule),
    Argonaut(argonaut::ArgonautModule),
    File(file::FileModule),
    Shell(shell::ShellModule),
    User(user::UserModule),
    Verify(verify::VerifyModule),
}

impl SutraModule for Module {
    fn name(&self) -> &str {
        match self {
            Module::Ark(m) => m.name(),
            Module::Argonaut(m) => m.name(),
            Module::File(m) => m.name(),
            Module::Shell(m) => m.name(),
            Module::User(m) => m.name(),
            Module::Verify(m) => m.name(),
        }
    }

    fn actions(&self) -> &[&str] {
        match self {
            Module::Ark(m) => m.actions(),
            Module::Argonaut(m) => m.actions(),
            Module::File(m) => m.actions(),
            Module::Shell(m) => m.actions(),
            Module::User(m) => m.actions(),
            Module::Verify(m) => m.actions(),
        }
    }

    async fn plan(
        &self,
        task: &Task,
        node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskPlan> {
        match self {
            Module::Ark(m) => m.plan(task, node, exec).await,
            Module::Argonaut(m) => m.plan(task, node, exec).await,
            Module::File(m) => m.plan(task, node, exec).await,
            Module::Shell(m) => m.plan(task, node, exec).await,
            Module::User(m) => m.plan(task, node, exec).await,
            Module::Verify(m) => m.plan(task, node, exec).await,
        }
    }

    async fn apply(
        &self,
        task: &Task,
        node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskResult> {
        match self {
            Module::Ark(m) => m.apply(task, node, exec).await,
            Module::Argonaut(m) => m.apply(task, node, exec).await,
            Module::File(m) => m.apply(task, node, exec).await,
            Module::Shell(m) => m.apply(task, node, exec).await,
            Module::User(m) => m.apply(task, node, exec).await,
            Module::Verify(m) => m.apply(task, node, exec).await,
        }
    }

    async fn check(&self, task: &Task, node: &NodeInfo, exec: &Executor) -> anyhow::Result<bool> {
        match self {
            Module::Ark(m) => m.check(task, node, exec).await,
            Module::Argonaut(m) => m.check(task, node, exec).await,
            Module::File(m) => m.check(task, node, exec).await,
            Module::Shell(m) => m.check(task, node, exec).await,
            Module::User(m) => m.check(task, node, exec).await,
            Module::Verify(m) => m.check(task, node, exec).await,
        }
    }
}

/// Registry of all available modules.
pub struct ModuleRegistry {
    modules: Vec<Module>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: vec![
                Module::Ark(ark::ArkModule),
                Module::Argonaut(argonaut::ArgonautModule),
                Module::File(file::FileModule),
                Module::Shell(shell::ShellModule),
                Module::User(user::UserModule),
                Module::Verify(verify::VerifyModule),
            ],
        }
    }

    pub fn get(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.name() == name)
    }

    pub fn list(&self) -> Vec<(&str, &[&str])> {
        self.modules
            .iter()
            .map(|m| (m.name(), m.actions()))
            .collect()
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_core_modules() {
        let reg = ModuleRegistry::new();
        assert!(reg.get("ark").is_some());
        assert!(reg.get("argonaut").is_some());
        assert!(reg.get("file").is_some());
        assert!(reg.get("shell").is_some());
        assert!(reg.get("user").is_some());
        assert!(reg.get("verify").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list() {
        let reg = ModuleRegistry::new();
        let modules = reg.list();
        assert_eq!(modules.len(), 6);
        let names: Vec<&str> = modules.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"ark"));
        assert!(names.contains(&"argonaut"));
        assert!(names.contains(&"shell"));
        assert!(names.contains(&"user"));
    }
}
