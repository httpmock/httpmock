use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::{collections::HashSet, fmt::Display};

use crate::{
    common::{
        data::{
            Diff, DiffResult, FunctionComparison, HttpMockRequest, KeyValueComparison,
            KeyValueComparisonAttribute, KeyValueComparisonKeyValuePair, Mismatch,
            RequestRequirements, SingleValueComparison, Tokenizer,
        },
        util::is_none_or_empty,
    },
    server::matchers::{comparators::ValueComparator, Matcher},
};

// ************************************************************************************************
// SingleValueMatcher
// ************************************************************************************************
pub(crate) struct SingleValueMatcher<S, T>
where
    S: Display,
    T: Display,
{
    pub entity_name: &'static str,
    pub matcher_method: &'static str,
    pub matching_strategy: MatchingStrategy,
    pub expectation: for<'a> fn(&'a RequestRequirements) -> Option<Vec<&'a S>>,
    pub request_value: fn(&HttpMockRequest) -> Option<T>,
    pub comparator: Box<dyn ValueComparator<S, T> + Send + Sync>,
    pub diff_with: Option<Tokenizer>,
    pub weight: usize,
}

impl<S, T> SingleValueMatcher<S, T>
where
    S: Display,
    T: Display,
{
    fn find_unmatched<'a>(
        &self,
        req_value: &Option<T>,
        mock_values: &Option<Vec<&'a S>>,
    ) -> Vec<&'a S> {
        let mock_values = match mock_values {
            None => return Vec::new(),
            Some(mv) => mv.to_vec(),
        };

        mock_values
            .into_iter()
            .filter(|e| !self.comparator.matches(&Some(e), &req_value.as_ref()))
            .collect()
    }
}

impl<S, T> Matcher for SingleValueMatcher<S, T>
where
    S: Display,
    T: Display,
{
    fn matches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> bool {
        let mock_value = (self.expectation)(mock);
        if is_none_or_empty(&mock_value) {
            return true;
        }

        let req_value = (self.request_value)(req);
        self.find_unmatched(&req_value, &mock_value).is_empty()
    }

    fn distance(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> usize {
        let mock_values = (self.expectation)(mock);
        if is_none_or_empty(&mock_values) {
            return 0;
        }

        let req_value = (self.request_value)(req);
        self.find_unmatched(&req_value, &mock_values)
            .into_iter()
            .map(|s| self.comparator.distance(&Some(s), &req_value.as_ref()))
            .map(|d| d * self.weight)
            .sum()
    }

    fn mismatches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> Vec<Mismatch> {
        let mock_value = (self.expectation)(mock);
        if is_none_or_empty(&mock_value) {
            return Vec::new();
        }

        let req_value = (self.request_value)(req);
        self.find_unmatched(&req_value, &mock_value)
            .into_iter()
            .map(|mock_value| {
                let mock_value = mock_value.to_string();
                let req_value = req_value.as_ref().map_or(String::new(), |v| v.to_string());
                Mismatch {
                    matcher_method: self.matcher_method.to_string(),
                    comparison: Some(SingleValueComparison {
                        operator: self.comparator.name().to_string(),
                        expected: mock_value.to_owned(),
                        actual: req_value.to_owned(),
                    }),
                    key_value_comparison: None,
                    function_comparison: None,
                    entity: self.entity_name.to_string(),
                    diff: self.diff_with.map(|t| diff_str(&mock_value, &req_value, t)),
                    best_match: false,
                    matching_strategy: Some(self.matching_strategy.clone()),
                }
            })
            .collect()
    }
}

pub enum KeyValueOperator {
    AND,
    NAND,
    NOR,
    OR,
    IMPLICATION,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MatchingStrategy {
    Presence,
    Absence,
}

// ************************************************************************************************
// MultiValueMatcher
// ************************************************************************************************
pub(crate) struct MultiValueMatcher<EK, EV, RK, RV>
where
    EK: Display,
    EV: Display,
    RK: Display,
    RV: Display,
{
    pub entity_name: &'static str,
    pub matcher_method: &'static str,
    pub operator: KeyValueOperator,
    pub expectation: for<'a> fn(&'a RequestRequirements) -> Option<Vec<(&'a EK, Option<&'a EV>)>>,
    pub request_value: fn(&HttpMockRequest) -> Option<Vec<(RK, Option<RV>)>>,
    pub matching_strategy: MatchingStrategy,
    pub key_required: bool,
    pub key_comparator: Box<dyn ValueComparator<EK, RK> + Send + Sync>,
    pub value_comparator: Box<dyn ValueComparator<EV, RV> + Send + Sync>,
    pub weight: usize,
}

impl<EK, EV, RK, RV> MultiValueMatcher<EK, EV, RK, RV>
where
    EK: Display,
    EV: Display,
    RK: Display,
    RV: Display,
{
    fn find_unmatched<'a>(
        &self,
        req_values: &[(RK, Option<RV>)],
        mock_values: &'a Vec<(&'a EK, Option<&'a EV>)>,
    ) -> Vec<&'a (&'a EK, Option<&'a EV>)> {
        mock_values
            .iter()
            .filter(|(ek, ev)| {
                if self.key_required {
                    let key_present = req_values.iter().any(|(rk, _): &(RK, Option<RV>)| {
                        self.key_comparator.matches(&Some(ek), &Some(rk))
                    });

                    if !key_present {
                        // We negate here, since we are filtering for "unmatched" expectations -> true = unmatched
                        return true;
                    }
                }

                let request_value_matches = |(rk, rv): &(RK, Option<RV>)| {
                    let key_matches = self.key_comparator.matches(&Some(ek), &Some(rk));
                    let value_matches = match (ev, rv) {
                        (Some(_), None) => false, // Mock required a value but none was present
                        (Some(ev), Some(rv)) => self.value_comparator.matches(&Some(ev), &Some(rv)),
                        _ => true,
                    };

                    match self.operator {
                        KeyValueOperator::NAND => !(key_matches && value_matches),
                        KeyValueOperator::AND => key_matches && value_matches,
                        KeyValueOperator::NOR => !(key_matches || value_matches),
                        KeyValueOperator::OR => key_matches || value_matches,
                        KeyValueOperator::IMPLICATION => !key_matches || value_matches,
                    }
                };

                let is_match = match self.matching_strategy {
                    MatchingStrategy::Absence => req_values.iter().all(request_value_matches),
                    MatchingStrategy::Presence => req_values.iter().any(request_value_matches),
                };

                // We negate here, since we are filtering for "unmatched" expectations -> true = unmatched
                !is_match
            })
            .collect()
    }

    fn find_best_match<'a>(
        &self,
        sk: &EK,
        sv: &Option<&EV>,
        req_values: &'a [(RK, Option<RV>)],
    ) -> Option<(&'a RK, &'a Option<RV>)> {
        if req_values.is_empty() {
            return None;
        }

        if let Some((fk, fv)) = req_values
            .iter()
            .find(|(k, _)| k.to_string() == sk.to_string())
        {
            return Some((fk, fv));
        }

        req_values
            .iter()
            .map(|(tk, tv)| {
                let key_distance = self.key_comparator.distance(&Some(sk), &Some(tk));
                let value_distance = self.value_comparator.distance(sv, &tv.as_ref());
                (tk, tv, key_distance + value_distance)
            })
            .min_by(|(_, _, d1), (_, _, d2)| d1.cmp(d2))
            .map(|(k, v, _)| (k, v))
    }
}

impl<EK, EV, RK, RV> Matcher for MultiValueMatcher<EK, EV, RK, RV>
where
    EK: Display,
    EV: Display,
    RK: Display,
    RV: Display,
{
    fn matches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> bool {
        let mock_values = (self.expectation)(mock).unwrap_or_default();
        if mock_values.is_empty() {
            return true;
        }

        let req_values = (self.request_value)(req).unwrap_or_default();
        self.find_unmatched(&req_values, &mock_values).is_empty()
    }

    fn distance(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> usize {
        let mock_values = (self.expectation)(mock).unwrap_or_default();
        if mock_values.is_empty() {
            return 0;
        }

        let req_values = (self.request_value)(req).unwrap_or_default();
        self.find_unmatched(&req_values, &mock_values)
            .iter()
            .map(|(k, v)| match self.find_best_match(k, v, &req_values) {
                None => {
                    self.key_comparator.distance(&Some(k), &None)
                        + self.value_comparator.distance(v, &None)
                }
                Some((bmk, bmv)) => {
                    self.key_comparator.distance(&Some(k), &Some(bmk))
                        + self.value_comparator.distance(v, &bmv.as_ref())
                }
            })
            .map(|d| d * self.weight)
            .sum()
    }

    fn mismatches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> Vec<Mismatch> {
        let mock_values = (self.expectation)(mock);
        if is_none_or_empty(&mock_values) {
            return Vec::new();
        }

        let mock_values = mock_values.unwrap_or_default();
        let req_values = (self.request_value)(req).unwrap_or_default();
        self.find_unmatched(&req_values, &mock_values)
            .iter() // Use iter to avoid ownership issues and unnecessary data moving.
            .map(|(k, v)| {
                let best_match = self.find_best_match(k, v, &req_values);
                Mismatch {
                    entity: self.entity_name.to_string(),
                    matcher_method: self.matcher_method.to_string(),
                    comparison: None,
                    function_comparison: None,
                    key_value_comparison: Some(KeyValueComparison {
                        key: Some(KeyValueComparisonAttribute {
                            operator: self.key_comparator.name().to_string(),
                            expected: k.to_string(),
                            actual: best_match.map(|(bmk, _)| bmk.to_string()),
                        }),
                        value: v.map(|v| KeyValueComparisonAttribute {
                            operator: self.value_comparator.name().to_string(),
                            expected: v.to_string(),
                            actual: best_match
                                .and_then(|(_, bmv)| bmv.as_ref().map(|bmv| bmv.to_string())),
                        }),
                        expected_count: None,
                        actual_count: None,
                        all: (req_values)
                            .iter()
                            .map(|(key, value)| KeyValueComparisonKeyValuePair {
                                key: key.to_string(),
                                value: value.as_ref().map(|v| v.to_string()),
                            })
                            .collect(),
                    }),
                    matching_strategy: Some(self.matching_strategy.clone()),
                    diff: None,
                    best_match: best_match.is_some(),
                }
            })
            .collect()
    }
}

// ************************************************************************************************
// MultiValueCountMatcher
// ************************************************************************************************
pub(crate) struct MultiValueCountMatcher<EK, EV, RK, RV>
where
    EK: Display,
    EV: Display,
    RK: Display,
    RV: Display,
{
    pub entity_name: &'static str,
    pub matcher_method: &'static str,
    pub expectation:
        for<'a> fn(&'a RequestRequirements) -> Option<Vec<(Option<&'a EK>, Option<&'a EV>, usize)>>,
    pub request_value: fn(&HttpMockRequest) -> Option<Vec<(RK, Option<RV>)>>,
    pub key_comparator: Box<dyn ValueComparator<EK, RK> + Send + Sync>,
    pub value_comparator: Box<dyn ValueComparator<EV, RV> + Send + Sync>,
    pub weight: usize,
}

impl<EK, EV, RK, RV> MultiValueCountMatcher<EK, EV, RK, RV>
where
    EK: Display,
    EV: Display,
    RK: Display,
    RV: Display,
{
    fn find_unmatched<'a>(
        &self,
        req_values: &[(RK, Option<RV>)],
        mock_values: &'a [(Option<&'a EK>, Option<&'a EV>, usize)],
    ) -> Vec<&'a (Option<&'a EK>, Option<&'a EV>, usize)> {
        let matched_idx = self.find_matched_indices(req_values, mock_values);
        self.filter_unmatched_indices(mock_values, &matched_idx)
    }

    fn find_matched_indices<'a>(
        &self,
        req_values: &[(RK, Option<RV>)],
        mock_values: &'a [(Option<&'a EK>, Option<&'a EV>, usize)],
    ) -> HashSet<usize> {
        let mut matched_idx = HashSet::new();

        for (idx, (ek, ev, count)) in mock_values.iter().enumerate() {
            let matches = self.count_matching_req_values(req_values, ek, ev);
            if matches == *count {
                matched_idx.insert(idx);
            }
        }

        matched_idx
    }

    fn count_matching_req_values(
        &self,
        req_values: &[(RK, Option<RV>)],
        ek: &Option<&EK>,
        ev: &Option<&EV>,
    ) -> usize {
        req_values
            .iter()
            .filter(|(rk, rv)| {
                let key_matches = match ek {
                    Some(ek) => self.key_comparator.matches(&Some(ek), &Some(rk)),
                    None => true, // No expectation => true
                };

                let value_matches = match (ev, rv) {
                    (Some(ev), Some(rv)) => self.value_comparator.matches(&Some(ev), &Some(rv)),
                    (Some(_), None) => false, // Expectation but no request value => false
                    (None, _) => true,        // No expectation => true
                };

                key_matches && value_matches
            })
            .count()
    }

    fn filter_unmatched_indices<'a>(
        &self,
        mock_values: &'a [(Option<&'a EK>, Option<&'a EV>, usize)],
        matched_idx: &HashSet<usize>,
    ) -> Vec<&'a (Option<&'a EK>, Option<&'a EV>, usize)> {
        mock_values
            .iter()
            .enumerate()
            .filter(|(i, _)| !matched_idx.contains(i))
            .map(|(_, item)| item)
            .collect()
    }
}

impl<EK, EV, RK, RV> Matcher for MultiValueCountMatcher<EK, EV, RK, RV>
where
    EK: Display,
    EV: Display,
    RK: Display,
    RV: Display,
{
    fn matches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> bool {
        let mock_values = (self.expectation)(mock).unwrap_or_default();
        if mock_values.is_empty() {
            return true;
        }

        let req_values = (self.request_value)(req).unwrap_or_default();
        self.find_unmatched(&req_values, &mock_values).is_empty()
    }

    fn distance(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> usize {
        let mock_values = (self.expectation)(mock).unwrap_or_default();
        if mock_values.is_empty() {
            return 0;
        }

        let req_values = (self.request_value)(req).unwrap_or_default();
        self.find_unmatched(&req_values, &mock_values)
            .iter()
            .map(|(k, v, c)| {
                let num_matching = self.count_matching_req_values(&req_values, k, v);
                num_matching.abs_diff(*c)
            })
            .map(|d| d * self.weight)
            .sum()
    }

    fn mismatches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> Vec<Mismatch> {
        let mock_values = (self.expectation)(mock).unwrap_or_default();
        if mock_values.is_empty() {
            return Vec::new();
        }

        let req_values = (self.request_value)(req).unwrap_or_default();
        self.find_unmatched(&req_values, &mock_values)
            .iter() // Use iter to avoid ownership issues and unnecessary data moving.
            .map(|(k, v, expected_count)| {
                let actual_count = self.count_matching_req_values(&req_values, k, v);
                Mismatch {
                    entity: self.entity_name.to_string(),
                    matcher_method: self.matcher_method.to_string(),
                    comparison: None,
                    key_value_comparison: Some(KeyValueComparison {
                        key: k.map(|k| KeyValueComparisonAttribute {
                            operator: self.key_comparator.name().to_string(),
                            expected: k.to_string(),
                            actual: None,
                        }),
                        value: v.map(|v| KeyValueComparisonAttribute {
                            operator: self.value_comparator.name().to_string(),
                            expected: v.to_string(),
                            actual: None,
                        }),
                        expected_count: Some(*expected_count),
                        actual_count: Some(actual_count),
                        all: (req_values)
                            .iter()
                            .map(|(key, value)| KeyValueComparisonKeyValuePair {
                                key: key.to_string(),
                                value: value.as_ref().map(|v| v.to_string()),
                            })
                            .collect(),
                    }),
                    matching_strategy: None,
                    function_comparison: None,
                    diff: None,
                    best_match: false,
                }
            })
            .collect()
    }
}

// ************************************************************************************************
// FunctionValueMatcher
// ************************************************************************************************
pub(crate) struct FunctionValueMatcher<S, T> {
    pub entity_name: &'static str,
    pub matcher_function: &'static str,
    pub expectation: for<'a> fn(&'a RequestRequirements) -> Option<Vec<&'a S>>,
    pub request_value: for<'a> fn(&'a HttpMockRequest) -> Option<&'a T>,
    pub comparator: Box<dyn ValueComparator<S, T> + Send + Sync>,
    pub weight: usize,
}

impl<S, T> FunctionValueMatcher<S, T> {
    fn get_unmatched(&self, req_value: &Option<&T>, mock_values: &Option<Vec<&S>>) -> Vec<usize> {
        let mock_values = match mock_values {
            None => return Vec::new(),
            Some(mv) => mv.to_vec(),
        };
        let req_value = match req_value {
            None => {
                return mock_values
                    .into_iter()
                    .enumerate()
                    .map(|(idx, _)| idx)
                    .collect()
            }
            Some(rv) => rv,
        };
        mock_values
            .into_iter()
            .enumerate()
            .filter(|(_idx, e)| !self.comparator.matches(&Some(e), &Some(req_value)))
            .map(|(idx, _e)| (idx))
            .collect()
    }
}

impl<S, T> Matcher for FunctionValueMatcher<S, T> {
    fn matches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> bool {
        let mock_values = (self.expectation)(mock);
        if is_none_or_empty(&mock_values) {
            return true;
        }

        let req_value = (self.request_value)(req);
        self.get_unmatched(&req_value, &mock_values).is_empty()
    }

    fn distance(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> usize {
        let mock_values = (self.expectation)(mock);
        if is_none_or_empty(&mock_values) {
            return 0;
        }

        let req_value = (self.request_value)(req);
        self.get_unmatched(&req_value, &mock_values).len() * self.weight
    }

    fn mismatches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> Vec<Mismatch> {
        let mock_values = (self.expectation)(mock);
        if is_none_or_empty(&mock_values) {
            return Vec::new();
        }

        let req_value = (self.request_value)(req);
        self.get_unmatched(&req_value, &mock_values)
            .into_iter()
            .map(|idx| Mismatch {
                entity: self.entity_name.to_string(),
                matcher_method: self.matcher_function.to_string(),
                function_comparison: Some(FunctionComparison { index: idx }),
                comparison: None,
                key_value_comparison: None,
                diff: None,
                best_match: false,
                matching_strategy: None,
            })
            .collect()
    }
}

#[inline]
pub fn diff_str(base: &str, edit: &str, tokenizer: Tokenizer) -> DiffResult {
    let changes = match tokenizer {
        Tokenizer::Line => TextDiff::from_lines(base, edit),
        Tokenizer::Word => TextDiff::from_words(base, edit),
        Tokenizer::Character => TextDiff::from_chars(base, edit),
    };

    DiffResult {
        tokenizer,
        distance: changes.ratio(),
        differences: changes
            .iter_all_changes()
            .map(|change| match change.tag() {
                ChangeTag::Equal => Diff::Same(change.to_string_lossy().to_string()),
                ChangeTag::Insert => Diff::Add(change.to_string_lossy().to_string()),
                ChangeTag::Delete => Diff::Rem(change.to_string_lossy().to_string()),
            })
            .collect(),
    }
}
