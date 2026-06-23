use anyhow::Result;
use referencing::{Registry, Resolver};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

use super::{schema::SchemaBuilderOptions, shared_context::SharedContext, RetrieveWrapper};

const DEFAULT_DRAFT: Draft = Draft::Draft202012;
const DEFAULT_ROOT_URI: &str = "json-schema:///";

pub use referencing::{Draft, ResourceRef};

fn draft_for(value: &Value) -> Draft {
    DEFAULT_DRAFT.detect(value)
}

pub struct PreContext {
    registry: Registry<'static>,
    draft: Draft,
    pub base_uri: String,
}

pub struct Context<'a> {
    resolver: Resolver<'a>,
    pub draft: Draft,
    pub shared: Rc<RefCell<SharedContext>>,
    pub options: SchemaBuilderOptions,
}

impl PreContext {
    pub fn new(contents: Value, retriever: Option<RetrieveWrapper>) -> Result<Self> {
        let draft = draft_for(&contents);
        let base_uri = draft
            .create_resource_ref(&contents)
            .id()
            .unwrap_or(DEFAULT_ROOT_URI)
            .to_string();
        let resource = draft.create_resource(contents);
        let registry_builder = Registry::new().draft(draft);
        let registry_builder = if let Some(retriever) = retriever {
            registry_builder.retriever(retriever)
        } else {
            registry_builder
        };
        let registry = registry_builder.add(&base_uri, resource)?.prepare()?;

        Ok(PreContext {
            registry,
            draft,
            base_uri,
        })
    }
}

impl<'a> Context<'a> {
    pub fn new(pre_context: &'a PreContext) -> Result<Self> {
        let resolver = pre_context
            .registry
            .resolver(referencing::uri::from_str(&pre_context.base_uri)?);
        let ctx = Context {
            resolver,
            draft: pre_context.draft,
            shared: Rc::new(RefCell::new(SharedContext::new())),
            options: SchemaBuilderOptions::default(),
        };

        Ok(ctx)
    }

    pub fn in_subresource(&'a self, resource: ResourceRef) -> Result<Context<'a>> {
        let resolver = self.resolver.in_subresource(resource)?;
        Ok(Context {
            resolver,
            draft: resource.draft(),
            shared: Rc::clone(&self.shared),
            options: self.options.clone(),
        })
    }

    pub fn as_resource_ref<'r>(&'a self, contents: &'r Value) -> ResourceRef<'r> {
        self.draft.detect(contents).create_resource_ref(contents)
    }

    pub fn normalize_ref(&self, reference: &str) -> Result<String> {
        Ok(self
            .resolver
            .resolve_uri(&self.resolver.base_uri().borrow(), reference)?
            .normalize()
            .into_string())
    }

    pub fn lookup_resource(&'a self, reference: &str) -> Result<ResourceRef<'a>> {
        let resolved = self.resolver.lookup(reference)?;
        Ok(self.as_resource_ref(resolved.contents()))
    }
}

impl referencing::Retrieve for RetrieveWrapper {
    fn retrieve(
        &self,
        uri: &referencing::Uri<String>,
    ) -> std::result::Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let value = self.0.retrieve(uri.as_str())?;
        Ok(value)
    }
}
