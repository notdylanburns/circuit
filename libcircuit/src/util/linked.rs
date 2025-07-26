pub struct LinkedStack<'a, T> {
    parent: Option<&'a Self>,
    item: T,
}

impl<'a, T> LinkedStack<'a, T> {
    pub fn new(item: T) -> Self {
        Self { parent: None, item }
    }

    pub fn push(&'a self, item: T) -> Self {
        Self {
            parent: Some(self),
            item: item,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::successors(Some(self), |stack| stack.parent).map(|stack| &stack.item)
    }

    pub fn find(&self, item: &T) -> Option<usize>
    where
        T: PartialEq,
    {
        self.iter()
            .enumerate()
            .find(|(_, i)| i == &item)
            .map(|(index, _)| index)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.iter().nth(index)
    }

    pub fn top(&self) -> &T {
        &self.item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linked_stack() {
        let stack = LinkedStack::new(1);
        assert_eq!(stack.iter().next(), Some(&1));
        {
            let stack = stack.push(2);
            assert_eq!(stack.iter().next(), Some(&2));

            {
                let stack = stack.push(3);
                let items: Vec<_> = stack.iter().collect();
                assert_eq!(items, vec![&3, &2, &1]);

                assert_eq!(items.find(&2), Some(1));
                assert_eq!(items.find(&3), Some(0));
                assert_eq!(items.find(&4), None);

                assert_eq!(items.get(1), Some(&2));
            }

            assert_eq!(stack.iter().next(), Some(&2));
            assert_eq!(items.find(&3), None);
        }

        assert_eq!(stack.iter().next(), Some(&1));
    }
}
