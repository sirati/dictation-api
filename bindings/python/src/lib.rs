use dictation_api::Connection;
use pyo3::prelude::*;
fn error(e: dictation_api::Error) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
}
#[pyclass]
struct BrokerConnection {
    connection: Connection,
}
#[pymethods]
impl BrokerConnection {
    #[new]
    #[pyo3(signature = (endpoint=None))]
    fn new(py: Python<'_>, endpoint: Option<String>) -> PyResult<Self> {
        py.detach(move || Connection::connect(endpoint.as_deref().map(std::path::Path::new)))
            .map(|connection| Self { connection })
            .map_err(error)
    }
    fn request(&self, py: Python<'_>, json: String) -> PyResult<String> {
        py.detach(|| self.connection.request_json(&json))
            .map_err(error)
    }
    fn next_event(&self, py: Python<'_>, timeout_ms: u32) -> PyResult<Option<String>> {
        py.detach(|| self.connection.next_event_json(timeout_ms))
            .map_err(error)
    }
    fn close(&self) {
        let _ = self.connection.close();
    }
}
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BrokerConnection>()?;
    Ok(())
}
