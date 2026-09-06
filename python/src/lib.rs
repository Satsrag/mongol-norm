//! Python bindings for the `mongol-norm` crate (`mongol_norm._native`).
//!
//! maturin compiles this crate into the private extension module `mongol_norm._native`.
//! `mongol_norm/_api.py` wraps it and reproduces the historical pure-Python API —
//! signatures, dict shapes, exception types and messages — so this layer stays thin: it
//! converts values and maps errors, nothing else. Argument validation that produces
//! Python-specific messages (`TypeError`s, `repr()` formatting) lives on the Python side.

#![forbid(unsafe_code)]

use std::str::FromStr;

use mongol_norm::{Error, Locale, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

pyo3::create_exception!(
    _native,
    FallbackError,
    PyValueError,
    "Strict normalization needed the fallback encoding; args are `(text, written_units)`."
);

type Names = Vec<&'static str>;
type Detail = (
    u32,
    Option<&'static str>,
    &'static str,
    Option<u8>,
    Option<&'static str>,
    Names,
);
type Change = (usize, Option<&'static str>, Option<&'static str>);
type Trace = (
    Names,
    Vec<(&'static str, Vec<Change>)>,
    Vec<Option<&'static str>>,
    Vec<Names>,
    Names,
);

fn names(units: &[WrittenUnit]) -> Names {
    units.iter().map(|unit| unit.as_str()).collect()
}

/// Map a core error onto the exception the pure-Python API raised for it.
fn to_py(err: Error) -> PyErr {
    let unsupported = matches!(err, Error::NormalizeUnsupported { .. });
    match err {
        Error::NormalizationFallback {
            text,
            written_units,
        } => FallbackError::new_err((text, names(&written_units))),
        // The pure-Python API pointed at the generator script; keep that hint.
        other if unsupported => PyRuntimeError::new_err(format!(
            "{other}; generate it with scripts/gen_normalize_table.py"
        )),
        other => PyValueError::new_err(other.to_string()),
    }
}

#[pyclass(name = "Shaper", frozen, module = "mongol_norm._native")]
struct PyShaper {
    inner: Shaper,
}

#[pymethods]
impl PyShaper {
    #[new]
    #[pyo3(signature = (locale = "MNG"))]
    fn new(locale: &str) -> PyResult<Self> {
        let locale = Locale::from_str(locale).map_err(to_py)?;
        Ok(PyShaper {
            inner: Shaper::new(locale),
        })
    }

    #[getter]
    fn locale(&self) -> &'static str {
        self.inner.locale().as_str()
    }

    /// `RuntimeError` for locales without a bundled normalize table (Python parity).
    fn canonical_version(&self) -> PyResult<&'static str> {
        self.inner.canonical_version().ok_or_else(|| {
            to_py(Error::NormalizeUnsupported {
                locale: self.inner.locale(),
            })
        })
    }

    fn rule_names(&self) -> Names {
        self.inner.rule_names()
    }

    fn shape(&self, text: &str) -> PyResult<Names> {
        self.inner
            .shape(text)
            .map(|units| names(&units))
            .map_err(to_py)
    }

    /// The engine's own written units, before the nine duplicate encodings are unified.
    /// Not part of the public contract (mirrors the core crate's `#[doc(hidden)] shape_raw`);
    /// it exists so the conformance suites can compare against GB/T 25914-2023 verbatim.
    fn shape_raw(&self, text: &str) -> PyResult<Names> {
        self.inner
            .shape_raw(text)
            .map(|units| names(&units))
            .map_err(to_py)
    }

    fn shape_str(&self, text: &str) -> PyResult<String> {
        self.inner.shape_str(text).map_err(to_py)
    }

    fn same_shape(&self, a: &str, b: &str) -> PyResult<bool> {
        self.inner.same_shape(a, b).map_err(to_py)
    }

    /// One `(cp, alias, position, fvs_index, condition, written)` tuple per token.
    fn shape_detailed(&self, text: &str) -> PyResult<Vec<Detail>> {
        let details = self.inner.shape_detailed(text).map_err(to_py)?;
        Ok(details
            .into_iter()
            .map(|detail| {
                (
                    u32::from(detail.cp),
                    detail.alias.map(|alias| alias.as_str()),
                    detail.position.as_str(),
                    detail.fvs.map(|fvs| fvs.index()),
                    detail.condition.map(|condition| condition.as_str()),
                    names(&detail.written),
                )
            })
            .collect())
    }

    /// `(positions, transitions, final_conditions, written_by_token, shape)`, where each
    /// transition is `(rule, [(token, before, after), …])`.
    fn trace(&self, text: &str) -> PyResult<Trace> {
        let trace = self.inner.trace(text).map_err(to_py)?;
        let transitions = trace
            .transitions
            .iter()
            .map(|transition| {
                let changes = transition
                    .changes
                    .iter()
                    .map(|change| {
                        (
                            change.token,
                            change.before.map(|condition| condition.as_str()),
                            change.after.map(|condition| condition.as_str()),
                        )
                    })
                    .collect();
                (transition.rule, changes)
            })
            .collect();
        Ok((
            trace
                .positions
                .iter()
                .map(|position| position.as_str())
                .collect(),
            transitions,
            trace
                .final_conditions
                .iter()
                .map(|condition| condition.map(|condition| condition.as_str()))
                .collect(),
            trace
                .written_by_token
                .iter()
                .map(|units| names(units))
                .collect(),
            names(&trace.shape),
        ))
    }

    #[pyo3(signature = (text, strict = true))]
    fn normalize(&self, text: &str, strict: bool) -> PyResult<String> {
        if strict {
            self.inner.normalize(text).map_err(to_py)
        } else {
            self.inner.normalize_allow_fallback(text).map_err(to_py)
        }
    }

    #[pyo3(signature = (text, strict = true))]
    fn normalize_text(&self, text: &str, strict: bool) -> PyResult<String> {
        if strict {
            self.inner.normalize_text(text).map_err(to_py)
        } else {
            self.inner
                .normalize_text_allow_fallback(text)
                .map_err(to_py)
        }
    }

    /// Names of every written unit the normalize table can encode (plus the controls).
    fn known_written_units(&self) -> PyResult<Names> {
        self.inner
            .known_written_units()
            .map(|units| names(&units))
            .map_err(to_py)
    }

    /// `(unit, position)` name pairs of the HUD positioned inventory.
    fn positioned_written_units(&self) -> PyResult<Vec<(&'static str, &'static str)>> {
        self.inner
            .positioned_written_units()
            .map(|records| {
                records
                    .iter()
                    .map(|(unit, position)| (unit.as_str(), position.as_str()))
                    .collect()
            })
            .map_err(to_py)
    }

    /// Python `normalize_written_units` after its argument validation: unit names.
    fn normalize_written_units(&self, units: Vec<String>) -> PyResult<String> {
        let units = units
            .iter()
            .map(|name| WrittenUnit::from_str(name).map_err(to_py))
            .collect::<PyResult<Vec<_>>>()?;
        self.inner.normalize_written_units(&units).map_err(to_py)
    }

    /// Python `normalize_positioned_written_units` after its validation: name pairs.
    fn normalize_positioned_written_units(
        &self,
        records: Vec<(String, String)>,
    ) -> PyResult<String> {
        let records = records
            .iter()
            .map(|(unit, position)| {
                Ok(PositionedWrittenUnit::new(
                    WrittenUnit::from_str(unit).map_err(to_py)?,
                    UnitPosition::from_str(position).map_err(to_py)?,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        self.inner
            .normalize_positioned_written_units(&records)
            .map_err(to_py)
    }

    fn parse_written_units(&self, text: &str) -> PyResult<Names> {
        self.inner
            .parse_written_units(text)
            .map(|units| names(&units))
            .map_err(to_py)
    }
}

/// The crate version (also the Python package version).
#[pyfunction]
fn version() -> &'static str {
    mongol_norm::version()
}

/// Run the `mongol-norm` CLI on `args` (without the program name); returns the exit status.
#[pyfunction]
fn cli_main(args: Vec<String>) -> i32 {
    mongol_norm::cli::run_args(&args)
}

/// Test hook: a shaper whose normalize table is empty, so every chain falls back.
#[cfg(feature = "testing")]
#[pyfunction]
fn _shaper_with_empty_normalize_table(locale: &str) -> PyResult<PyShaper> {
    let locale = Locale::from_str(locale).map_err(to_py)?;
    Ok(PyShaper {
        inner: Shaper::with_empty_normalize_table(locale),
    })
}

#[pymodule(name = "_native")]
fn native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyShaper>()?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(cli_main, m)?)?;
    #[cfg(feature = "testing")]
    m.add_function(wrap_pyfunction!(_shaper_with_empty_normalize_table, m)?)?;
    m.add("FallbackError", m.py().get_type::<FallbackError>())?;
    Ok(())
}
