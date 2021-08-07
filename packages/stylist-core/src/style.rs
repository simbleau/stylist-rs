use once_cell::sync::OnceCell;
use std::borrow::Borrow;
use std::convert::TryInto;
use std::result::Result;
use std::sync::Arc;

use crate::ast::{Scopes, ToCss};
use crate::registry::{StyleKey, StyleRegistry};
use crate::utils::get_rand_str;
#[cfg(target_arch = "wasm32")]
use crate::utils::{doc_head, document};
#[cfg(target_arch = "wasm32")]
use crate::Error;

#[derive(Debug)]
struct StyleContent {
    key: Arc<StyleKey>,

    /// The designated class name of this style
    class_name: String,

    /// The abstract syntax tree of the css
    ast: Arc<Scopes>,

    style_str: OnceCell<String>,
}

impl StyleContent {
    fn get_class_name(&self) -> &str {
        &self.class_name
    }

    fn get_style_str(&self) -> &str {
        self.style_str
            .get_or_init(|| self.ast.to_css(self.get_class_name()))
    }

    /// Mounts the styles to the document
    #[cfg(target_arch = "wasm32")]
    fn mount(&self) -> Result<()> {
        let document = document()?;
        let head = doc_head()?;

        let style_element = document
            .create_element("style")
            .map_err(|e| Error::Web(Some(e)))?;
        style_element
            .set_attribute("data-style", self.get_class_name())
            .map_err(|e| Error::Web(Some(e)))?;
        style_element.set_text_content(Some(self.get_style_str()));

        head.append_child(&style_element)
            .map_err(|e| Error::Web(Some(e)))?;
        Ok(())
    }

    /// Unmounts the style from the DOM tree
    /// Does nothing if it's not in the DOM tree
    #[cfg(target_arch = "wasm32")]
    fn unmount(&self) -> Result<()> {
        let document = document()?;

        if let Some(m) = document
            .query_selector(&format!("style[data-style={}]", self.class_name))
            .map_err(|e| Error::Web(Some(e)))?
        {
            if let Some(parent) = m.parent_element() {
                parent.remove_child(&m).map_err(|e| Error::Web(Some(e)))?;
            }
        }

        Ok(())
    }

    fn key(&self) -> Arc<StyleKey> {
        self.key.clone()
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for StyleContent {
    /// Unmounts the style from the HTML head web-sys style
    fn drop(&mut self) {
        let _result = self.unmount();
    }
}

/// A struct that represents a scoped Style.
#[derive(Debug, Clone)]
pub struct Style {
    inner: Arc<StyleContent>,
}

impl Style {
    // The big method is monomorphic, so less code duplication and code bloat through generics
    // and inlining
    fn create_from_scopes_impl(class_prefix: &str, css: Scopes) -> Self {
        let css = Arc::new(css);
        // Creates the StyleKey, return from registry if already cached.
        let key = StyleKey(css);
        let reg = StyleRegistry::get_ref();
        let mut reg = reg.lock().unwrap();

        if let Some(m) = reg.get(&key) {
            return m;
        }

        let new_style = Self {
            inner: Arc::new(StyleContent {
                class_name: format!("{}-{}", class_prefix, get_rand_str()),
                ast: key.0.clone(),
                style_str: OnceCell::new(),
                key: Arc::new(key),
            }),
        };

        #[cfg(target_arch = "wasm32")]
        new_style.inner.mount().expect("Failed to mount");

        // Register the created Style.
        reg.register(new_style.clone());

        new_style
    }

    /// Creates a new style
    ///
    /// # Examples
    ///
    /// ```
    /// use stylist_core::{Style, ast::Scopes};
    ///
    /// let scopes: Scopes = Default::default();
    /// let style = Style::try_from_scopes(scopes)?;
    /// # Ok::<(), std::convert::Infallible>(())
    /// ```
    pub fn try_from_scopes<S: TryInto<Scopes>>(css: S) -> Result<Self, S::Error> {
        let css = S::try_into(css)?;
        Ok(Self::create_from_scopes("stylist", css))
    }

    /// Creates a new style with custom class prefix
    ///
    /// # Examples
    ///
    /// ```
    /// use stylist_core::Style;
    ///
    /// let scopes = Default::default();
    /// let style = Style::create_from_scopes("my-component", scopes);
    /// ```
    pub fn create_from_scopes<I: Borrow<str>>(class_prefix: I, css: Scopes) -> Self {
        Self::create_from_scopes_impl(class_prefix.borrow(), css)
    }

    /// Returns the class name for current style
    ///
    /// You can add this class name to the element to apply the style.
    ///
    /// # Examples
    ///
    /// ```
    /// use stylist_core::Style;
    ///
    /// let scopes = Default::default();
    /// let style = Style::create_from_scopes("stylist", scopes);
    ///
    /// // Example Output: stylist-uSu9NZZu
    /// println!("{}", style.get_class_name());
    /// ```
    pub fn get_class_name(&self) -> &str {
        self.inner.get_class_name()
    }

    /// Get the parsed and generated style in `&str`.
    ///
    /// This is usually used for debug purposes or testing in non-wasm32 targets.
    ///
    /// # Examples
    ///
    /// ```
    /// use stylist_core::Style;
    ///
    /// let scopes = Default::default();
    /// let style = Style::create_from_scopes("my-component", scopes);
    ///
    /// // Example Output:
    /// // .my-component-uSu9NZZu {
    /// // color: red;
    /// // }
    /// println!("{}", style.get_style_str());
    /// ```
    pub fn get_style_str(&self) -> &str {
        self.inner.get_style_str()
    }

    /// Return a reference of style key.
    pub(crate) fn key(&self) -> Arc<StyleKey> {
        self.inner.key()
    }

    /// Unregister current style from style registry
    ///
    /// After calling this method, the style will be unmounted from DOM after all its clones are freed.
    pub fn unregister(&self) {
        let reg = StyleRegistry::get_ref();
        let mut reg = reg.lock().unwrap();
        reg.unregister(&*self.key());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sample_scopes;

    #[test]
    fn test_simple() {
        Style::try_from_scopes(sample_scopes()).expect("Failed to create Style.");
    }
}
