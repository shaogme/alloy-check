use std::collections::{HashMap, HashSet};
use syn::visit::{self, Visit};

/// 记录工作空间内定义的符号信息。
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// 按包名索引的内部固有函数（顶层函数）。
    pub free_fns: HashMap<String, HashSet<String>>,
    /// 按包名索引的内部固有方法（非 Trait impl 里的方法）。
    pub inherent_methods: HashMap<String, HashSet<String>>,
    /// 工作空间内定义的所有 Trait 方法名。
    pub trait_methods: HashSet<String>,
}

impl SymbolIndex {
    /// 判定一个调用目标是否属于当前包的内部非 Trait 逻辑。
    /// `is_method` 表示调用形式是否为方法调用（x.g()）。
    pub fn is_internal_logic(&self, package_name: &str, ident: &str, is_method: bool) -> bool {
        // 如果它是 Trait 方法，不论在哪里定义都允许别名
        if self.trait_methods.contains(ident) {
            return false;
        }

        let bucket = if is_method {
            &self.inherent_methods
        } else {
            &self.free_fns
        };

        if let Some(symbols) = bucket.get(package_name)
            && symbols.contains(ident)
        {
            return true;
        }

        false
    }
}

pub(crate) struct IndexVisitor<'a> {
    pub free_fns: &'a mut HashSet<String>,
    pub inherent_methods: &'a mut HashSet<String>,
    pub trait_methods: &'a mut HashSet<String>,
}

impl<'ast> Visit<'ast> for IndexVisitor<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.free_fns.insert(i.sig.ident.to_string());
        visit::visit_item_fn(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let is_trait = i.trait_.is_some();
        for item in &i.items {
            if let syn::ImplItem::Fn(m) = item {
                let name = m.sig.ident.to_string();
                if is_trait {
                    self.trait_methods.insert(name);
                } else {
                    self.inherent_methods.insert(name);
                }
            }
        }
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        for item in &i.items {
            if let syn::TraitItem::Fn(m) = item {
                self.trait_methods.insert(m.sig.ident.to_string());
            }
        }
        visit::visit_item_trait(self, i);
    }
}
