minimer::define_app_error!(pub AppError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_keeps_the_message() {
        let error: AppError = AppError::new("members are out of order");

        assert_eq!(error.message, "Error: members are out of order");
    }
}
