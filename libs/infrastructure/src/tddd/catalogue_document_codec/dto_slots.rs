//! Entry-slot serde DTOs for the [`CatalogueDocument`] wire format.
//!
//! A `types` / `traits` / `functions` map value is either a live entry or a
//! deletion tombstone. [`EntrySlotDto`] discriminates the two on the entry's
//! `action` field, and [`TombstoneDto`] carries deletion identity plus grounding.
//!
//! All types in this module are infrastructure-private (`pub(super)`).

use serde::{Deserialize, Serialize, de};
use serde_json::{Map, Number, Value};

use crate::tddd::spec_ground_codec::{InformalGroundRefDto, SpecRefDto};

// ---------------------------------------------------------------------------
// Deletion tombstone DTO
// ---------------------------------------------------------------------------

/// Wire format for a deletion tombstone `action: delete` entry (spec IN-04 / GO-03 /
/// AC-04).
///
/// A deletion has no live shape: the map key supplies the name (or, for
/// functions, the full path) and the body carries at most a `module_path` plus
/// the normal grounding fields. `deny_unknown_fields` rejects any `role` /
/// `kind` / `methods` / `docs`, so a `delete` entry cannot smuggle live-entry
/// annotations. The decoder routes it into `CatalogueDocument::deletions`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TombstoneDto {
    /// Always `"delete"`; the discriminator the decoder peeks. Kept as a field so
    /// the entry stays self-describing and round-trips byte-for-byte.
    pub(super) action: String,
    /// Crate-relative module path of the removed item. Empty for a crate-root
    /// item or a function (whose module is embedded in the map key).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(super) module_path: String,
    /// Formal spec grounds for the deletion.
    #[serde(default)]
    pub(super) spec_refs: Vec<SpecRefDto>,
    /// Informal grounds for deletion work not yet promoted to spec refs.
    #[serde(default)]
    pub(super) informal_grounds: Vec<InformalGroundRefDto>,
}

// ---------------------------------------------------------------------------
// Entry slot — live entry vs deletion tombstone
// ---------------------------------------------------------------------------

/// JSON value used only for entry-slot dispatch.
///
/// `serde_json::Value` accepts duplicate object keys with last-wins semantics.
/// Entry dispatch must preserve the codec's fail-closed duplicate-key behavior,
/// so this wrapper recursively rejects duplicate keys before the value is passed
/// to the strict typed DTO deserializers.
struct StrictJsonValue(Value);

impl<'de> Deserialize<'de> for StrictJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StrictJsonValueVisitor;

        impl<'de> de::Visitor<'de> for StrictJsonValueVisitor {
            type Value = StrictJsonValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON value without duplicate object keys")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue(Value::Bool(value)))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue(Value::Number(Number::from(value))))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue(Value::Number(Number::from(value))))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let number = Number::from_f64(value)
                    .ok_or_else(|| E::custom("invalid floating-point JSON number"))?;
                Ok(StrictJsonValue(Value::Number(number)))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue(Value::String(value.to_owned())))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue(Value::String(value)))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue(Value::Null))
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                StrictJsonValue::deserialize(deserializer)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictJsonValue(Value::Null))
            }

            fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = access.next_element::<StrictJsonValue>()? {
                    values.push(value.0);
                }
                Ok(StrictJsonValue(Value::Array(values)))
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut object = Map::new();
                while let Some(key) = access.next_key::<String>()? {
                    if object.contains_key(&key) {
                        return Err(de::Error::custom(format!(
                            "duplicate key in catalogue entry object: {key}"
                        )));
                    }
                    let value = access.next_value::<StrictJsonValue>()?;
                    object.insert(key, value.0);
                }
                Ok(StrictJsonValue(Value::Object(object)))
            }
        }

        deserializer.deserialize_any(StrictJsonValueVisitor)
    }
}

/// One slot in a `types` / `traits` / `functions` map: either a live entry `T`
/// or a deletion [`TombstoneDto`].
///
/// Discriminated by the entry's `action` field: `"delete"` decodes as
/// [`Self::Tombstone`], anything else (or an absent `action`, which defaults to
/// `add`) as [`Self::Live`]. Serialization delegates to the inner value, so a
/// live entry keeps its declaration-order field layout (byte-stable) and is
/// never wrapped.
#[derive(Debug)]
pub(super) enum EntrySlotDto<T> {
    /// A live entry (`add` / `modify` / `reference`).
    Live(T),
    /// A deletion (`action: delete`).
    Tombstone(TombstoneDto),
}

impl<T: Serialize> Serialize for EntrySlotDto<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            EntrySlotDto::Live(entry) => entry.serialize(serializer),
            EntrySlotDto::Tombstone(tombstone) => tombstone.serialize(serializer),
        }
    }
}

impl<'de, T> Deserialize<'de> for EntrySlotDto<T>
where
    T: de::DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Peek the `action` discriminator on a materialised value, then decode
        // the appropriate typed DTO. Duplicate keys are rejected recursively
        // before materialisation, and `deny_unknown_fields` on both branches is
        // preserved because the re-decode goes through the derived `Deserialize`.
        let value = StrictJsonValue::deserialize(deserializer)?.0;
        let is_delete = value.get("action").and_then(Value::as_str) == Some("delete");
        if is_delete {
            serde_json::from_value::<TombstoneDto>(value)
                .map(EntrySlotDto::Tombstone)
                .map_err(de::Error::custom)
        } else {
            serde_json::from_value::<T>(value).map(EntrySlotDto::Live).map_err(de::Error::custom)
        }
    }
}
