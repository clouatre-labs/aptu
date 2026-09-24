// SPDX-License-Identifier: Apache-2.0

//! Helpers for emitting GitHub Actions workflow commands (`::error ...::`).
//!
//! Workflow commands require percent-encoding of control characters:
//! data after `::` must escape `%` as `%25`, CR as `%0D`, and LF as `%0A`;
//! property values (before `::`) additionally escape `:` as `%3A` and `,` as
//! `%2C` because they delimit key/value pairs.

/// Escapes workflow-command data (the message after `::`).
pub fn escape_workflow_data(data: &str) -> String {
    data.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Escapes a workflow-command property value (for example `title`).
pub fn escape_workflow_property(value: &str) -> String {
    escape_workflow_data(value)
        .replace(':', "%3A")
        .replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_data_percent_cr_lf() {
        assert_eq!(
            escape_workflow_data("100% done\r\nnext"),
            "100%25 done%0D%0Anext"
        );
    }

    #[test]
    fn test_escape_property_escapes_colon_and_comma() {
        assert_eq!(
            escape_workflow_property("a:b, c%d\ne"),
            "a%3Ab%2C c%25d%0Ae"
        );
    }
}
