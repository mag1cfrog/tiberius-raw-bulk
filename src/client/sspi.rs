use std::{ffi::c_void, io, ptr, slice};

use windows_sys::{
    core::HRESULT,
    Win32::{
        Foundation::{SEC_E_OK, SEC_I_CONTINUE_NEEDED},
        Security::{
            Authentication::Identity::{
                AcquireCredentialsHandleW, DeleteSecurityContext, FreeContextBuffer,
                FreeCredentialsHandle, InitializeSecurityContextW, SecBuffer, SecBufferDesc,
                ISC_REQ_ALLOCATE_MEMORY, ISC_REQ_CONFIDENTIALITY, ISC_REQ_CONNECTION,
                ISC_REQ_DELEGATE, ISC_REQ_INTEGRITY, ISC_REQ_REPLAY_DETECT,
                ISC_REQ_SEQUENCE_DETECT, ISC_REQ_USE_SESSION_KEY, SECBUFFER_TOKEN,
                SECBUFFER_VERSION, SECPKG_CRED_OUTBOUND, SECURITY_NATIVE_DREP,
            },
            Credentials::SecHandle,
        },
        System::Rpc::{SEC_WINNT_AUTH_IDENTITY_UNICODE, SEC_WINNT_AUTH_IDENTITY_W},
    },
};

const NTLM_PACKAGE: &str = "NTLM";

const INIT_REQUEST_FLAGS: u32 = ISC_REQ_CONFIDENTIALITY
    | ISC_REQ_INTEGRITY
    | ISC_REQ_REPLAY_DETECT
    | ISC_REQ_SEQUENCE_DETECT
    | ISC_REQ_CONNECTION
    | ISC_REQ_DELEGATE
    | ISC_REQ_USE_SESSION_KEY
    | ISC_REQ_ALLOCATE_MEMORY;

pub(crate) struct SspiClient {
    credentials: CredentialHandle,
    context: Option<SecurityContext>,
    target_spn: Vec<u16>,
}

impl SspiClient {
    pub(crate) fn integrated(target_spn: &str) -> io::Result<Self> {
        Ok(Self {
            credentials: CredentialHandle::acquire(None)?,
            context: None,
            target_spn: wide_null(target_spn),
        })
    }

    pub(crate) fn with_credentials(
        target_spn: &str,
        domain: Option<String>,
        user: String,
        password: String,
    ) -> io::Result<Self> {
        let mut identity = AuthIdentity::new(domain, user, password);

        Ok(Self {
            credentials: CredentialHandle::acquire(Some(identity.as_ptr()))?,
            context: None,
            target_spn: wide_null(target_spn),
        })
    }

    pub(crate) fn next_bytes(&mut self, input: Option<&[u8]>) -> io::Result<Option<Vec<u8>>> {
        let mut input_buffer = sec_buffer(SECBUFFER_TOKEN, input);
        let input_desc = sec_buffer_desc(&mut input_buffer);

        let mut output_buffer = sec_buffer(SECBUFFER_TOKEN, None);
        let mut output_desc = sec_buffer_desc(&mut output_buffer);

        let mut context_attrs = 0;
        let mut first_context = SecHandle::default();

        let (input_context, output_context) = match self.context.as_mut() {
            Some(context) => (
                &context.0 as *const SecHandle,
                &mut context.0 as *mut SecHandle,
            ),
            None => (ptr::null(), &mut first_context as *mut SecHandle),
        };

        let status = unsafe {
            InitializeSecurityContextW(
                &self.credentials.0,
                input_context,
                self.target_spn.as_ptr(),
                INIT_REQUEST_FLAGS,
                0,
                SECURITY_NATIVE_DREP,
                &input_desc,
                0,
                output_context,
                &mut output_desc,
                &mut context_attrs,
                ptr::null_mut(),
            )
        };

        match status {
            SEC_E_OK | SEC_I_CONTINUE_NEEDED => {
                if self.context.is_none() {
                    self.context = Some(SecurityContext(first_context));
                }

                let output = output_token(&output_buffer)?;

                if status == SEC_I_CONTINUE_NEEDED && output.is_none() {
                    return Err(sspi_error(
                        status,
                        "InitializeSecurityContextW requested continuation without a token",
                    ));
                }

                Ok(output)
            }
            status => {
                let _ = free_output_token(&output_buffer);
                Err(sspi_error(status, "InitializeSecurityContextW"))
            }
        }
    }
}

struct CredentialHandle(SecHandle);

impl CredentialHandle {
    fn acquire(auth_data: Option<*const c_void>) -> io::Result<Self> {
        let package = wide_null(NTLM_PACKAGE);
        let mut credentials = SecHandle::default();
        let status = unsafe {
            AcquireCredentialsHandleW(
                ptr::null(),
                package.as_ptr(),
                SECPKG_CRED_OUTBOUND,
                ptr::null(),
                auth_data.unwrap_or(ptr::null()),
                None,
                ptr::null(),
                &mut credentials,
                ptr::null_mut(),
            )
        };

        if status == SEC_E_OK {
            Ok(Self(credentials))
        } else {
            Err(sspi_error(status, "AcquireCredentialsHandleW"))
        }
    }
}

impl Drop for CredentialHandle {
    fn drop(&mut self) {
        unsafe {
            FreeCredentialsHandle(&self.0);
        }
    }
}

struct SecurityContext(SecHandle);

impl Drop for SecurityContext {
    fn drop(&mut self) {
        unsafe {
            DeleteSecurityContext(&self.0);
        }
    }
}

struct AuthIdentity {
    _user: Vec<u16>,
    _domain: Vec<u16>,
    _password: Vec<u16>,
    raw: SEC_WINNT_AUTH_IDENTITY_W,
}

impl AuthIdentity {
    fn new(domain: Option<String>, user: String, password: String) -> Self {
        let mut user = wide(user.as_str());
        let mut domain = wide(domain.as_deref().unwrap_or_default());
        let mut password = wide(password.as_str());

        let raw = SEC_WINNT_AUTH_IDENTITY_W {
            User: user.as_mut_ptr(),
            UserLength: user.len() as u32,
            Domain: domain.as_mut_ptr(),
            DomainLength: domain.len() as u32,
            Password: password.as_mut_ptr(),
            PasswordLength: password.len() as u32,
            Flags: SEC_WINNT_AUTH_IDENTITY_UNICODE,
        };

        Self {
            _user: user,
            _domain: domain,
            _password: password,
            raw,
        }
    }

    fn as_ptr(&mut self) -> *const c_void {
        &mut self.raw as *mut SEC_WINNT_AUTH_IDENTITY_W as *const c_void
    }
}

fn sec_buffer(buffer_type: u32, bytes: Option<&[u8]>) -> SecBuffer {
    let (cb_buffer, pv_buffer) = match bytes {
        Some(bytes) => (bytes.len() as u32, bytes.as_ptr() as *mut c_void),
        None => (0, ptr::null_mut()),
    };

    SecBuffer {
        cbBuffer: cb_buffer,
        BufferType: buffer_type,
        pvBuffer: pv_buffer,
    }
}

fn sec_buffer_desc(buffer: &mut SecBuffer) -> SecBufferDesc {
    SecBufferDesc {
        ulVersion: SECBUFFER_VERSION,
        cBuffers: 1,
        pBuffers: buffer,
    }
}

fn output_token(buffer: &SecBuffer) -> io::Result<Option<Vec<u8>>> {
    if buffer.cbBuffer == 0 || buffer.pvBuffer.is_null() {
        return Ok(None);
    }

    let token = unsafe {
        slice::from_raw_parts(buffer.pvBuffer as *const u8, buffer.cbBuffer as usize).to_vec()
    };

    free_output_token(buffer)?;

    Ok(Some(token))
}

fn free_output_token(buffer: &SecBuffer) -> io::Result<()> {
    if buffer.pvBuffer.is_null() {
        return Ok(());
    }

    let status = unsafe { FreeContextBuffer(buffer.pvBuffer) };

    if status == SEC_E_OK {
        Ok(())
    } else {
        Err(sspi_error(status, "FreeContextBuffer"))
    }
}

fn sspi_error(status: HRESULT, operation: &str) -> io::Error {
    io::Error::other(format!(
        "{operation} failed with SSPI status 0x{:08X}",
        status as u32
    ))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
