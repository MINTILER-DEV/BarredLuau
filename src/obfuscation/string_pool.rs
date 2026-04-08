use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct StringPool {
    values: Vec<String>,
    reverse: BTreeMap<String, usize>,
}

impl StringPool {
    pub fn intern(&mut self, value: impl Into<String>) -> usize {
        let value = value.into();
        if let Some(index) = self.reverse.get(&value) {
            return *index;
        }
        let index = self.values.len();
        self.values.push(value.clone());
        self.reverse.insert(value, index);
        index
    }

    pub fn values(&self) -> &[String] {
        &self.values
    }
}
