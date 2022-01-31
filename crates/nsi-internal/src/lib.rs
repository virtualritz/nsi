#[cfg(feature = "nsi-toolbelt")]
/// Scene construction helpers.
pub mod toolbelt {
    pub use nsi_toolbelt::*;
}

#[cfg(feature = "nsi-jupyter")]
/// [Jupyter notebook](https://jupyter.org/) integration.
pub mod jupyter {
    pub use nsi_jupyter::*;
}

#[cfg(feature = "nsi-3delight")]
/// [3Delight](https://www.3delight.com/) specific helpers.
pub mod delight {
    pub use nsi_3delight::*;
}
