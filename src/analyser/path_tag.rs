use crate::tokeniser::IdentId;
use std::rc::Rc;

pub(super) struct PathTagged<T> {
    path: Rc<[IdentId]>,
    value: T,
}

impl<T> std::ops::Deref for PathTagged<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> std::ops::DerefMut for PathTagged<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: Clone> Clone for PathTagged<T> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            value: self.value.clone(),
        }
    }
}

impl<T: PartialEq> PartialEq for PathTagged<T> {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.value == other.value
    }
}

impl<T: Eq> Eq for PathTagged<T> {}

impl<T> PathTagged<T> {
    pub fn path(&self) -> &[IdentId] {
        &self.path
    }

    pub fn path_clone(&self) -> Rc<[IdentId]> {
        self.path.clone()
    }

    pub fn map<U, F>(self, f: F) -> PathTagged<U>
    where
        F: FnOnce(T) -> U,
    {
        PathTagged {
            path: self.path,
            value: f(self.value),
        }
    }

    pub fn map_ref<U, F>(&self, f: F) -> PathTagged<U>
    where
        F: FnOnce(&T) -> U,
    {
        PathTagged {
            path: self.path.clone(),
            value: f(&self.value),
        }
    }
}

pub(super) trait PathTag: Sized {
    fn tag_empty(self) -> PathTagged<Self> {
        self.tag_path(Rc::from([]))
    }

    fn tag_path(self, path: Rc<[IdentId]>) -> PathTagged<Self> {
        PathTagged { path, value: self }
    }
}

impl<T: Sized> PathTag for T {}
