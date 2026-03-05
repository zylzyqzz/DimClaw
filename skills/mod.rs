mod shell_command;
mod types;

use std::collections::HashMap;
use std::sync::Arc;

pub use types::{Skill, SkillContext, SkillResult};

#[derive(Clone)]
pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        let mut skills: HashMap<String, Arc<dyn Skill>> = HashMap::new();
        skills.insert("shell_command".to_string(), Arc::new(shell_command::ShellCommandSkill));
        Self { skills }
    }
}

impl SkillRegistry {
    pub fn get(&self, name: &str) -> Option<Arc<dyn Skill>> {
        self.skills.get(name).cloned()
    }
}
