minimer::define_app_error!(pub AppError);

// For work moved to another thread, which needs a `Send` result; keeps the message but no source error.
minimer::define_app_error_static!(pub AppErrorStatic);

impl From<AppError> for AppErrorStatic {
    fn from(error: AppError) -> AppErrorStatic {
        AppErrorStatic(minimer::AppErrorStatic::from(error.0))
    }
}

impl From<AppErrorStatic> for AppError {
    fn from(error: AppErrorStatic) -> AppError {
        AppError(minimer::AppError::from(error.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_keeps_the_message() {
        let error: AppError = AppError::new("members are out of order");

        assert_eq!(error.message, "Error: members are out of order");
    }

    #[test]
    fn app_error_static_from_keeps_the_message_and_the_source_text() {
        let source_error: std::io::Error = std::io::Error::other("disk full");
        let error: AppError = AppError::from_error("cannot write the replay", Box::new(source_error));
        let static_error: AppErrorStatic = AppErrorStatic::from(error);

        assert_eq!(static_error.message, "Error: cannot write the replay -> disk full");
    }

    #[test]
    fn app_error_from_keeps_the_message_of_a_static_error() {
        let static_error: AppErrorStatic = AppErrorStatic::new("hashing failed");
        let error: AppError = AppError::from(static_error);

        assert_eq!(error.message, "Error: hashing failed");
    }
}
