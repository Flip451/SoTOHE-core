//! Method implementations for `FullyQualifiedItemPath`.
//!
//! The enum itself is defined in the parent `identifiers` module so that its
//! rustdoc path equals the catalogue-declared module path; this child module
//! only carries the (large) inherent impl block.

use crate::tddd::semantic_verify::CatalogueEntryKey;

use super::helpers::split_catalogue_key;
use super::{
    CatalogueItemNamespace, CrateName, FullyQualifiedItemPath, Identifier, IdentifierError,
    ModulePath,
};

impl FullyQualifiedItemPath {
    /// Creates a placed type identity from validated path components.
    #[must_use]
    pub fn new_type(crate_name: CrateName, module_path: ModulePath, name: Identifier) -> Self {
        Self::PlacedType { crate_name, module_path, name }
    }

    /// Creates a placed trait identity from validated path components.
    #[must_use]
    pub fn new_trait(crate_name: CrateName, module_path: ModulePath, name: Identifier) -> Self {
        Self::PlacedTrait { crate_name, module_path, name }
    }

    /// Creates an unplaced type identity.
    #[must_use]
    pub fn new_unplaced_type(crate_name: CrateName, name: Identifier) -> Self {
        Self::UnplacedType { crate_name, name }
    }

    /// Creates an unplaced trait identity.
    #[must_use]
    pub fn new_unplaced_trait(crate_name: CrateName, name: Identifier) -> Self {
        Self::UnplacedTrait { crate_name, name }
    }

    /// Creates a placed type identity for compatibility with existing callers.
    #[must_use]
    pub fn new(crate_name: CrateName, module_path: ModulePath, name: Identifier) -> Self {
        Self::new_type(crate_name, module_path, name)
    }

    /// Returns the crate name component.
    #[must_use]
    pub fn crate_name(&self) -> &CrateName {
        match self {
            Self::PlacedType { crate_name, .. }
            | Self::UnplacedType { crate_name, .. }
            | Self::PlacedTrait { crate_name, .. }
            | Self::UnplacedTrait { crate_name, .. } => crate_name,
        }
    }

    /// Returns the explicitly declared module path, if placement is known.
    #[must_use]
    pub fn module_path(&self) -> Option<&ModulePath> {
        match self {
            Self::PlacedType { module_path, .. } | Self::PlacedTrait { module_path, .. } => {
                Some(module_path)
            }
            Self::UnplacedType { .. } | Self::UnplacedTrait { .. } => None,
        }
    }

    /// Returns the item namespace.
    #[must_use]
    pub fn namespace(&self) -> CatalogueItemNamespace {
        match self {
            Self::PlacedType { .. } | Self::UnplacedType { .. } => CatalogueItemNamespace::Type,
            Self::PlacedTrait { .. } | Self::UnplacedTrait { .. } => CatalogueItemNamespace::Trait,
        }
    }

    /// Returns whether this identity carries an explicit placement.
    #[must_use]
    pub fn is_placed(&self) -> bool {
        self.module_path().is_some()
    }

    /// Returns the declared item name component.
    #[must_use]
    pub fn name(&self) -> &Identifier {
        match self {
            Self::PlacedType { name, .. }
            | Self::UnplacedType { name, .. }
            | Self::PlacedTrait { name, .. }
            | Self::UnplacedTrait { name, .. } => name,
        }
    }

    /// Resolves a type catalogue key while preserving omitted placement.
    ///
    /// # Errors
    ///
    /// Returns an identifier error when a key segment is malformed.
    pub fn from_type_catalogue_entry_key(
        crate_name: &CrateName,
        key: &CatalogueEntryKey,
        declared_module_path: Option<&ModulePath>,
    ) -> Result<Self, IdentifierError> {
        Self::from_catalogue_key(
            crate_name,
            key,
            declared_module_path,
            CatalogueItemNamespace::Type,
        )
    }

    /// Resolves a trait catalogue key while preserving omitted placement.
    ///
    /// # Errors
    ///
    /// Returns an identifier error when a key segment is malformed.
    pub fn from_trait_catalogue_entry_key(
        crate_name: &CrateName,
        key: &CatalogueEntryKey,
        declared_module_path: Option<&ModulePath>,
    ) -> Result<Self, IdentifierError> {
        Self::from_catalogue_key(
            crate_name,
            key,
            declared_module_path,
            CatalogueItemNamespace::Trait,
        )
    }

    /// Resolves a legacy catalogue key as a placed type identity.
    ///
    /// This compatibility entry point retains the historical API for callers
    /// that already have an explicit module path. New code should use the
    /// namespace-specific constructors above.
    pub fn from_catalogue_entry_key(
        crate_name: &CrateName,
        key: &CatalogueEntryKey,
        declared_module_path: &ModulePath,
    ) -> Result<Self, IdentifierError> {
        Self::from_type_catalogue_entry_key(crate_name, key, Some(declared_module_path))
    }

    /// Parses a fully qualified type key.
    ///
    /// # Errors
    ///
    /// Returns an identifier error when the key is bare or malformed.
    pub fn from_type_fully_qualified_key(key: &CatalogueEntryKey) -> Result<Self, IdentifierError> {
        Self::from_fully_qualified_key_in_namespace(key, CatalogueItemNamespace::Type)
    }

    /// Parses a fully qualified trait key.
    ///
    /// # Errors
    ///
    /// Returns an identifier error when the key is bare or malformed.
    pub fn from_trait_fully_qualified_key(
        key: &CatalogueEntryKey,
    ) -> Result<Self, IdentifierError> {
        Self::from_fully_qualified_key_in_namespace(key, CatalogueItemNamespace::Trait)
    }

    /// Parses a fully qualified key as a placed type identity for compatibility.
    ///
    /// # Errors
    ///
    /// Returns an identifier error when the key is bare or malformed.
    pub fn from_fully_qualified_key(key: &CatalogueEntryKey) -> Result<Self, IdentifierError> {
        Self::from_type_fully_qualified_key(key)
    }

    fn from_catalogue_key(
        crate_name: &CrateName,
        key: &CatalogueEntryKey,
        declared_module_path: Option<&ModulePath>,
        namespace: CatalogueItemNamespace,
    ) -> Result<Self, IdentifierError> {
        let (item_name, path_segments) = split_catalogue_key(key)?;
        let name = Identifier::new(item_name.to_owned())?;
        let module_segments = match path_segments.split_first() {
            Some((first, rest)) if *first == crate_name.as_str() || *first == "crate" => rest,
            Some(_) => path_segments.as_slice(),
            None => {
                return Ok(Self::from_namespace(
                    namespace,
                    crate_name.clone(),
                    declared_module_path.cloned(),
                    name,
                ));
            }
        };
        let module_path = ModulePath::from_segments(
            module_segments.iter().map(|segment| (*segment).to_owned()).collect(),
        )?;
        Ok(Self::from_namespace(namespace, crate_name.clone(), Some(module_path), name))
    }

    fn from_fully_qualified_key_in_namespace(
        key: &CatalogueEntryKey,
        namespace: CatalogueItemNamespace,
    ) -> Result<Self, IdentifierError> {
        let (item_name, path_segments) = split_catalogue_key(key)?;
        let (crate_segment, module_segments) = path_segments
            .split_first()
            .ok_or_else(|| IdentifierError::InvalidFunctionPath(key.as_str().to_owned()))?;
        let crate_name = CrateName::new((*crate_segment).to_owned())?;
        let name = Identifier::new(item_name.to_owned())?;
        let module_path = ModulePath::from_segments(
            module_segments.iter().map(|segment| (*segment).to_owned()).collect(),
        )?;
        Ok(Self::from_namespace(namespace, crate_name, Some(module_path), name))
    }

    fn from_namespace(
        namespace: CatalogueItemNamespace,
        crate_name: CrateName,
        module_path: Option<ModulePath>,
        name: Identifier,
    ) -> Self {
        match (namespace, module_path) {
            (CatalogueItemNamespace::Type, Some(module_path)) => {
                Self::new_type(crate_name, module_path, name)
            }
            (CatalogueItemNamespace::Type, None) => Self::new_unplaced_type(crate_name, name),
            (CatalogueItemNamespace::Trait, Some(module_path)) => {
                Self::new_trait(crate_name, module_path, name)
            }
            (CatalogueItemNamespace::Trait, None) => Self::new_unplaced_trait(crate_name, name),
        }
    }
}
