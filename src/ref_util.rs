use std::sync::Arc;
use std::cell::{RefCell, Ref as OtherRef, RefMut};

#[derive(Clone, Debug)]
pub struct Ref<T> {
    data: Arc<RefCell<T>>,
}

impl<T> Ref<T> {
    pub fn with<R>(&self, callable: impl Fn(OtherRef<T>) -> R) -> R {
        let data_ref = self.data
            .try_borrow()
            .expect("Failed to immutably borrow");

        callable(data_ref) 
    }

    pub fn with_mut<R>(&mut self, mut callable: impl FnMut(RefMut<T>) -> R) -> R {
        let data_ref = self.data
            .try_borrow_mut()
            .expect("Failed to mutably borrow");

        callable(data_ref)
    }
}

impl<T> PartialEq for Ref<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}

pub trait MakeRef<T> {
    fn into_ref(self) -> Ref<T>;
    fn make_ref(value: T) -> Ref<T>;
}

impl<T> MakeRef<T> for T {
    fn into_ref(self) -> Ref<T> {
        Self::make_ref(self)
    }

    fn make_ref(value: T) -> Ref<T> {
        Ref {
            data: Arc::new(RefCell::new(value))
        }
    }
}

#[test]
fn test_eq() {
    #[derive(Clone, Debug)]
    struct Test {
        field: usize,
    };

    let mystruct = Test {
        field: 1234,
    };

    let struct_ref = mystruct.into_ref();

    let struct_ref2 = struct_ref.clone();

    assert_eq!(struct_ref, struct_ref2);
}