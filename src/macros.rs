
#[macro_export]
macro_rules! command {
    ($fmt:expr) => ( command!($fmt,) );
    ($fmt:expr, $( $id:ident = $value:expr ),* $(,)*) => (
        {
            $crate::commandify(
                &format!($fmt, $( $id = $crate::command_arg(&$value) ),*)
            )
        }
    );
}


#[macro_export]
macro_rules! execute {
    ($fmt:expr) => ( execute!($fmt,) );
    ($fmt:expr, $( $id:ident = $value:expr ),* $(,)*) => (
        {
            use $crate::{CommandSpecExt};
            command!($fmt, $( $id = $value ),*).unwrap().execute()
        }
    );
}

#[macro_export]
macro_rules! sh_command {
    ($fmt:expr) => ( sh_command!($fmt,) );
    ($fmt:expr, $( $id:ident = $value:expr ),* $(,)*) => (
        $crate::commandify(
            &format!(
                "sh -c {}",
                $crate::command_arg(
                    &format!("set -e\n\n{}", format!($fmt, $( $id = $crate::command_arg(&$value) ,)*)),
                ),
            )
        )
    );
}

#[macro_export]
macro_rules! sush_command {
    ($fmt:expr) => ( sush_command!($fmt,) );
    ($fmt:expr, $( $id:ident = $value:expr ),* $(,)*) => (
        $crate::commandify(
            &format!(
                "pkexec sh -c {}",
                $crate::command_arg(
                    &format!("set -e\n\n{}", format!($fmt, $( $id = $crate::command_arg(&$value) ,)*)),
                ),
            )
        )
    );
}

#[macro_export]
macro_rules! sh_execute {
    ($fmt:expr) => ( sh_execute!($fmt,) );
    ($fmt:expr, $( $id:ident = $value:expr ),* $(,)*) => (
        {
            use $crate::{CommandSpecExt};
            sh_command!($fmt, $( $id = $value ),*).unwrap().execute()
        }
    );
}

#[macro_export]
macro_rules! sush_execute {
    ($fmt:expr) => ( sush_execute!($fmt,) );
    ($fmt:expr, $( $id:ident = $value:expr ),* $(,)*) => (
        {
            use $crate::{CommandSpecExt};
            sush_command!($fmt, $( $id = $value ),*).unwrap().execute()
        }
    );
}