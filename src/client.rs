use std::{
    ffi::{c_char, c_void, CStr, CString},
    ptr,
};

use log::{debug, error, info, trace, warn};
use open62541_sys::{
    va_list, UA_ClientConfig, UA_ClientConfig_setDefault, UA_Client_connect, UA_Client_getConfig,
    UA_LogCategory, UA_LogLevel, UA_LogLevel_UA_LOGLEVEL_DEBUG, UA_LogLevel_UA_LOGLEVEL_ERROR,
    UA_LogLevel_UA_LOGLEVEL_FATAL, UA_LogLevel_UA_LOGLEVEL_INFO, UA_LogLevel_UA_LOGLEVEL_TRACE,
    UA_LogLevel_UA_LOGLEVEL_WARNING, UA_STATUSCODE_GOOD,
};

#[cfg(feature = "tokio")]
use crate::AsyncClient;
use crate::{ua, Error};

/// Builder for [`Client`].
///
/// Use this to specify additional options before connecting to an OPC UA endpoint.
#[allow(clippy::module_name_repetitions)]
pub struct ClientBuilder(ua::Client);

impl ClientBuilder {
    /// Connects to OPC UA endpoint and returns [`Client`].
    ///
    /// # Errors
    ///
    /// This fails when the target server is not reachable.
    ///
    /// # Panics
    ///
    /// The endpoint URL must be a valid C string, i.e. it must not contain any NUL bytes.
    pub fn connect(mut self, endpoint_url: &str) -> Result<Client, Error> {
        info!("Connecting to endpoint {endpoint_url}");

        let endpoint_url =
            CString::new(endpoint_url).expect("endpoint URL does not contain NUL bytes");

        let result = unsafe { UA_Client_connect(self.0.as_mut_ptr(), endpoint_url.as_ptr()) };
        if result != UA_STATUSCODE_GOOD {
            return Err(Error::new(result));
        }

        Ok(Client(self.0))
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        let mut inner = ua::Client::default();

        // We require some initial configuration `UA_Client_connect()` to work.
        //
        let result = unsafe {
            let config = UA_Client_getConfig(inner.as_mut_ptr());

            // Install custom logger that uses `log` crate.
            set_default_logger(config.as_mut().expect("client config should be set"));

            // Setting the remainder of the configuration to defaults keeps our custom logger. Do so
            // after setting the logger to prevent this call to install another default logger which
            // we would throw away in `set_default_logger()` anyway.
            UA_ClientConfig_setDefault(config)
        };
        assert!(result == UA_STATUSCODE_GOOD);

        Self(inner)
    }
}

/// Connected OPC UA client.
///
/// This represents an OPC UA client connected to a specific endpoint. Once a client is connected to
/// an endpoint, it is not possible to switch to another server. Create a new client for that.
///
/// Once a connection to the given endpoint is established, the client keeps the connection open and
/// reconnects if necessary.
///
/// If the connection fails unrecoverably, the client is no longer usable. In this case create a new
/// client if required.
pub struct Client(ua::Client);

impl Client {
    /// Creates client connected to endpoint.
    ///
    /// If you need more control over the initialization, use [`ClientBuilder`] instead, and turn it
    /// into [`Client`] by calling [`connect()`](ClientBuilder::connect).
    ///
    /// # Errors
    ///
    /// See [`ClientBuilder::connect()`].
    ///
    /// # Panics
    ///
    /// See [`ClientBuilder::connect()`].
    pub fn new(endpoint_url: &str) -> Result<Self, Error> {
        ClientBuilder::default().connect(endpoint_url)
    }

    /// Turns client into [`AsyncClient`].
    ///
    /// The [`AsyncClient`] can be used to access methods in an asynchronous way.
    #[must_use]
    #[cfg(feature = "tokio")]
    pub fn into_async(self) -> AsyncClient {
        AsyncClient::from_sync(self.0)
    }
}

/// Installs logger that forwards to `log` crate.
///
/// This remove an existing logger from the given configuration (by calling its `clear()` callback),
/// then installs a custom logger that forwards all messages to the corresponding calls in the `log`
/// crate.
///
/// We can use this to prevent `open62541` from installing its own default logger (which outputs any
/// logs to stdout/stderr directly).
fn set_default_logger(config: &mut UA_ClientConfig) {
    unsafe extern "C" fn log_c(
        _log_context: *mut c_void,
        level: UA_LogLevel,
        _category: UA_LogCategory,
        msg: *const c_char,
        _args: va_list,
    ) {
        let msg = unsafe { CStr::from_ptr(msg) }.to_string_lossy();

        if level == UA_LogLevel_UA_LOGLEVEL_FATAL {
            // There is no fatal level  in `log`, use `error`.
            error!("{msg}");
        } else if level == UA_LogLevel_UA_LOGLEVEL_ERROR {
            error!("{msg}");
        } else if level == UA_LogLevel_UA_LOGLEVEL_WARNING {
            warn!("{msg}");
        } else if level == UA_LogLevel_UA_LOGLEVEL_INFO {
            info!("{msg}");
        } else if level == UA_LogLevel_UA_LOGLEVEL_DEBUG {
            debug!("{msg}");
        } else if level == UA_LogLevel_UA_LOGLEVEL_TRACE {
            trace!("{msg}");
        } else {
            // TODO: Handle unexpected level.
        }
    }

    // Reset existing logger configuration.
    if let Some(clear) = config.logger.clear {
        unsafe { clear(config.logger.context) };
    }

    // Set logger configuration to our own.
    config.logger.clear = None;
    config.logger.log = Some(log_c);
    config.logger.context = ptr::null_mut();
}
