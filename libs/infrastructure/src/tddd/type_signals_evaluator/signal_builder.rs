//! Conversion of three-way evaluator output into persisted type signals.

use super::*;

/// Intermediate accumulator entry for a single top-level item.
///
/// Fields: `(signal, found_type, found_items, missing_items, extra_items)`.
type AccEntry = (ConfidenceSignal, bool, Vec<String>, Vec<String>, Vec<String>);

pub(super) fn build_type_signals_from_report<'a>(
    signals: impl Iterator<Item = &'a ThreeWaySignal>,
    kind_tag_map: &BTreeMap<String, Vec<&'static str>>,
) -> Vec<TypeSignal> {
    use domain::tddd::signal_evaluator::region::SignalRegion;

    let mut acc: HashMap<String, AccEntry> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for signal in signals {
        let name = signal.item_name();
        let confidence = signal_kind_to_confidence(signal.signal());
        let found_in_c = !matches!(
            signal.region(),
            SignalRegion::SMinusC_Add
                | SignalRegion::SMinusC_Modify
                | SignalRegion::SMinusC_Reference
                | SignalRegion::DMinusC
        );

        if let Some(sep) = name.find(": ") {
            let owner = &name[..sep];
            let trait_part = &name[sep + 2..];
            let entry = acc.entry(owner.to_owned()).or_insert_with(|| {
                order.push(owner.to_owned());
                (ConfidenceSignal::Blue, true, Vec::new(), Vec::new(), Vec::new())
            });

            if signal.region() != SignalRegion::DMinusC {
                entry.0 = worse_signal(entry.0, confidence);
                match signal.region() {
                    SignalRegion::SIntersectC_Match_Add
                    | SignalRegion::SIntersectC_Match_Modify => {
                        entry.2.push(trait_part.to_owned());
                    }
                    SignalRegion::CMinusSUnionD => {
                        entry.4.push(trait_part.to_owned());
                    }
                    _ => {
                        entry.3.push(trait_part.to_owned());
                    }
                }
            }
        } else {
            let entry = acc.entry(name.to_owned()).or_insert_with(|| {
                order.push(name.to_owned());
                (confidence, found_in_c, Vec::new(), Vec::new(), Vec::new())
            });
            entry.0 = worse_signal(entry.0, confidence);
            entry.1 = entry.1 || found_in_c;
        }
    }

    for name in kind_tag_map.keys() {
        acc.entry(name.clone()).or_insert_with(|| {
            order.push(name.clone());
            (ConfidenceSignal::Blue, true, Vec::new(), Vec::new(), Vec::new())
        });
    }

    order
        .into_iter()
        .flat_map(|name| {
            let Some((sig, found_type, found_items, missing_items, extra_items)) =
                acc.remove(&name)
            else {
                return Vec::new();
            };
            let kind_tags = kind_tag_map.get(name.as_str()).map(Vec::as_slice).unwrap_or(&[]);
            if kind_tags.is_empty() {
                return vec![TypeSignal::new(
                    name,
                    "unknown",
                    sig,
                    found_type,
                    found_items,
                    missing_items,
                    extra_items,
                )];
            }
            let is_collision = kind_tags.len() > 1;
            kind_tags
                .iter()
                .map(|&kind_tag| {
                    let effective_signal = if is_collision {
                        worse_signal(sig, ConfidenceSignal::Yellow)
                    } else {
                        sig
                    };
                    TypeSignal::new(
                        name.clone(),
                        kind_tag,
                        effective_signal,
                        found_type,
                        found_items.clone(),
                        missing_items.clone(),
                        extra_items.clone(),
                    )
                })
                .collect()
        })
        .collect()
}

fn signal_kind_to_confidence(kind: ThreeWaySignalKind) -> ConfidenceSignal {
    match kind {
        ThreeWaySignalKind::Blue => ConfidenceSignal::Blue,
        ThreeWaySignalKind::Yellow => ConfidenceSignal::Yellow,
        ThreeWaySignalKind::Red => ConfidenceSignal::Red,
        ThreeWaySignalKind::Skip => ConfidenceSignal::Yellow,
    }
}

fn worse_signal(a: ConfidenceSignal, b: ConfidenceSignal) -> ConfidenceSignal {
    match (a, b) {
        (ConfidenceSignal::Red, _) | (_, ConfidenceSignal::Red) => ConfidenceSignal::Red,
        (ConfidenceSignal::Yellow, _) | (_, ConfidenceSignal::Yellow) => ConfidenceSignal::Yellow,
        _ => ConfidenceSignal::Blue,
    }
}
