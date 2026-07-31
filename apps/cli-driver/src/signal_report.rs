//! Typed filter conversion for the `signal report` primary-adapter boundary.

use usecase::signal_report::{
    SignalReportChain, SignalReportChainSelection, SignalReportLevelSelection, SignalReportQuery,
};

/// A transport-level filter selecting report chains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalReportChainFilter {
    /// Include every report chain.
    All,
    /// Include only the ADR-to-user chain.
    AdrUser,
    /// Include only the specification-to-ADR chain.
    SpecAdr,
    /// Include only the catalogue-to-specification chain.
    CatalogSpec,
    /// Include only the implementation-to-catalogue chain.
    ImplCatalog,
}

/// A transport-level filter selecting report signal levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalReportLevelFilter {
    /// Include only yellow occurrences.
    YellowOnly,
    /// Include only red occurrences.
    RedOnly,
    /// Include both yellow and red occurrences.
    YellowAndRed,
}

/// Typed `signal report` input accepted by the CLI driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalReportInput {
    /// The requested report-chain filter.
    pub chain: SignalReportChainFilter,
    /// The requested signal-level filter.
    pub levels: SignalReportLevelFilter,
}

#[allow(dead_code)] // Consumed by SignalReportDriver when T004 adds the driver.
fn to_signal_report_query(input: SignalReportInput) -> SignalReportQuery {
    let chain = match input.chain {
        SignalReportChainFilter::All => SignalReportChainSelection::All,
        SignalReportChainFilter::AdrUser => {
            SignalReportChainSelection::One(SignalReportChain::AdrUser)
        }
        SignalReportChainFilter::SpecAdr => {
            SignalReportChainSelection::One(SignalReportChain::SpecAdr)
        }
        SignalReportChainFilter::CatalogSpec => {
            SignalReportChainSelection::One(SignalReportChain::CatalogSpec)
        }
        SignalReportChainFilter::ImplCatalog => {
            SignalReportChainSelection::One(SignalReportChain::ImplCatalog)
        }
    };
    let levels = match input.levels {
        SignalReportLevelFilter::YellowOnly => SignalReportLevelSelection::YellowOnly,
        SignalReportLevelFilter::RedOnly => SignalReportLevelSelection::RedOnly,
        SignalReportLevelFilter::YellowAndRed => SignalReportLevelSelection::YellowAndRed,
    };

    SignalReportQuery { chain, levels }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_report_input_each_chain_filter_converts_to_expected_selection() {
        let cases = [
            (SignalReportChainFilter::All, SignalReportChainSelection::All),
            (
                SignalReportChainFilter::AdrUser,
                SignalReportChainSelection::One(SignalReportChain::AdrUser),
            ),
            (
                SignalReportChainFilter::SpecAdr,
                SignalReportChainSelection::One(SignalReportChain::SpecAdr),
            ),
            (
                SignalReportChainFilter::CatalogSpec,
                SignalReportChainSelection::One(SignalReportChain::CatalogSpec),
            ),
            (
                SignalReportChainFilter::ImplCatalog,
                SignalReportChainSelection::One(SignalReportChain::ImplCatalog),
            ),
        ];

        for (filter, expected_chain) in cases {
            let query = to_signal_report_query(SignalReportInput {
                chain: filter,
                levels: SignalReportLevelFilter::YellowAndRed,
            });

            assert_eq!(query.chain, expected_chain);
            assert_eq!(query.levels, SignalReportLevelSelection::YellowAndRed);
        }
    }

    #[test]
    fn test_signal_report_input_each_level_filter_converts_to_expected_selection() {
        let cases = [
            (SignalReportLevelFilter::YellowOnly, SignalReportLevelSelection::YellowOnly),
            (SignalReportLevelFilter::RedOnly, SignalReportLevelSelection::RedOnly),
            (SignalReportLevelFilter::YellowAndRed, SignalReportLevelSelection::YellowAndRed),
        ];

        for (filter, expected_levels) in cases {
            let query = to_signal_report_query(SignalReportInput {
                chain: SignalReportChainFilter::All,
                levels: filter,
            });

            assert_eq!(query.chain, SignalReportChainSelection::All);
            assert_eq!(query.levels, expected_levels);
        }
    }
}
