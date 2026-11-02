//! Historique annuler/rétablir par instantanés : chaque modification réussie est appliquée sur une
//! copie, de sorte qu'une modification en échec ne laisse aucune trace.

use crate::project::Project;

#[derive(Debug, Clone, Default)]
pub struct Editor {
    pub project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
}

impl Editor {
    pub fn new(project: Project) -> Self {
        Editor { project, undo: Vec::new(), redo: Vec::new() }
    }

    /// Applique `f` ; en cas de succès l'état précédent rejoint l'historique et le « rétablir » est vidé.
    pub fn edit<R, E>(&mut self, f: impl FnOnce(&mut Project) -> Result<R, E>) -> Result<R, E> {
        let mut next = self.project.clone();
        let r = f(&mut next)?;
        if next != self.project {
            self.undo.push(std::mem::replace(&mut self.project, next));
            self.redo.clear();
        }
        Ok(r)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.project, prev));
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.project, next));
                true
            }
            None => false,
        }
    }
}
