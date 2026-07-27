//! Serde view of a convention document's front matter.
//!
//! Kept in a sibling module so that the codec, the directory scan, and the
//! adapter that injects it stay inside the parent module's length limit. The
//! types are re-exported from the parent, which is where the catalogue declares
//! them and where every consumer names them.

use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use usecase::conventions_resolve::{
    ConventionCapabilityId, ConventionCapabilityIdError, ConventionDocumentPath,
    ConventionRequirement, ConventionResolveError,
};

/// One `required_for` element as the document spelled it (`AC-06`, `AC-09`).
///
/// The validated identifier [`ConventionCapabilityId`] is a serde-free usecase
/// value, so the wire element is mirrored by this type instead. Keeping the
/// mirror distinct is what leaves the two `AC-07` conditions it sits between
/// separately observable: a non-string element fails as a shape error while
/// this field is being deserialized, an empty element fails later in
/// [`ConventionFrontMatterDto::into_requirement`], whereas a plain `String`
/// would collapse both into one untyped parse failure.
///
/// The held text is unvalidated and reaches [`ConventionCapabilityId::try_new`]
/// exactly as written: this type trims nothing and folds no case, because any
/// normalisation here would silently decide matches the usecase comparison is
/// supposed to decide.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CapabilityIdField(String);

impl CapabilityIdField {
    /// Borrows the wire text verbatim.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Front-matter key this codec reads. Every other entry is ignored.
const REQUIRED_FOR_KEY: &str = "required_for";

/// The `required_for` value, accepted only as an actual sequence.
///
/// Deserializing the elements straight into a `Vec` would accept a YAML null as
/// an empty sequence — `serde_yaml` coerces `required_for:`, `required_for:
/// null`, and `required_for: ~` into the same empty vector `required_for: []`
/// produces. That collapses the two states `AC-07` and `AC-08` keep apart: a
/// document declaring **no** `required_for` is a normal empty state, while a
/// `required_for` present with a null value is one of the shapes that is not an
/// array of strings, and so is fail-closed. Requiring a sequence here is what
/// keeps the absent key the only route to the empty declaration list.
struct RequiredForElements(Vec<CapabilityIdField>);

impl<'de> Deserialize<'de> for RequiredForElements {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ElementsVisitor;

        impl<'de> Visitor<'de> for ElementsVisitor {
            type Value = RequiredForElements;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an array of capability id strings")
            }

            fn visit_seq<A>(self, mut elements: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut declared = Vec::new();
                while let Some(element) = elements.next_element()? {
                    declared.push(element);
                }
                Ok(RequiredForElements(declared))
            }
        }

        // `deserialize_any` rather than `deserialize_seq`: the coercion happens
        // inside the format's own `deserialize_seq`, so asking for a sequence is
        // asking for the very conversion this type exists to refuse. Letting the
        // value announce its own type instead means a null arrives as the unit
        // this visitor does not accept.
        deserializer.deserialize_any(ElementsVisitor)
    }
}

/// Serde view of a convention document's YAML front matter (`AC-06`).
///
/// Only `required_for` is read. Every other entry is tolerated by design,
/// whatever type its key has, because consumer documents own their own metadata
/// and this codec has no business rejecting entries it does not read.
///
/// [`ConventionFrontMatterDto::default`] is also what a document with no
/// front-matter block decodes to, so `AC-08`'s two absent-metadata states share
/// one representation and no caller has to distinguish them.
#[derive(Debug, Default)]
pub struct ConventionFrontMatterDto {
    pub(super) required_for: Vec<CapabilityIdField>,
}

impl<'de> Deserialize<'de> for ConventionFrontMatterDto {
    /// Reads `required_for` and ignores every other entry.
    ///
    /// A derived implementation would resolve each key as a field identifier
    /// before any value is read, so a mapping carrying a non-string key —
    /// `true: metadata`, which YAML admits — would fail while that key was
    /// being resolved, before `required_for` had been examined at all. The
    /// caller cannot tell such a failure apart from a malformed `required_for`,
    /// so it would be reported as the wrong `AC-07` condition, and a document
    /// declaring no `required_for` would fail where `AC-08` promises the
    /// default value. Matching the key as a value instead keeps the tolerance
    /// independent of the key's type, and leaves `required_for`'s own shape the
    /// only decode failure reachable here.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FrontMatterVisitor;

        impl<'de> Visitor<'de> for FrontMatterVisitor {
            type Value = ConventionFrontMatterDto;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a convention front-matter mapping")
            }

            fn visit_map<A>(self, mut entries: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut required_for = Vec::new();
                while let Some(key) = entries.next_key::<serde_yaml::Value>()? {
                    if key.as_str() == Some(REQUIRED_FOR_KEY) {
                        required_for = entries.next_value::<RequiredForElements>()?.0;
                    } else {
                        entries.next_value::<IgnoredAny>()?;
                    }
                }
                Ok(ConventionFrontMatterDto { required_for })
            }
        }

        deserializer.deserialize_map(FrontMatterVisitor)
    }
}

impl ConventionFrontMatterDto {
    /// Pairs the decoded declarations with the document they came from.
    ///
    /// This method does not decide the empty-identifier condition:
    /// [`ConventionCapabilityId::try_new`] is its enforcing site, and there is
    /// no way to produce an identifier here except by calling it. What the
    /// method owns is the translation — it maps that constructor's rejection
    /// onto [`ConventionResolveError::EmptyCapabilityId`] and supplies the
    /// document, which the constructor is never handed.
    ///
    /// Takes `self` by value so no caller can keep the unvalidated elements
    /// once the validated requirement exists.
    ///
    /// # Errors
    ///
    /// Returns [`ConventionResolveError::EmptyCapabilityId`] when any element
    /// is empty or whitespace-only.
    pub fn into_requirement(
        self,
        document: ConventionDocumentPath,
    ) -> Result<ConventionRequirement, ConventionResolveError> {
        let declared: Result<Vec<ConventionCapabilityId>, ConventionCapabilityIdError> = self
            .required_for
            .iter()
            .map(|field| ConventionCapabilityId::try_new(field.as_str()))
            .collect();

        match declared {
            Ok(required_for) => Ok(ConventionRequirement::new(document, required_for)),
            Err(ConventionCapabilityIdError::Blank) => {
                Err(ConventionResolveError::EmptyCapabilityId { document })
            }
        }
    }
}
