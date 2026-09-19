use zed_extension_api::{self as zed, settings::LspSettings};

struct LocusExtension;

impl zed::Extension for LocusExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        let (path, args, env) = settings.binary.map_or_else(
            || (None, Vec::new(), Vec::new()),
            |binary| {
                (
                    binary.path,
                    binary.arguments.unwrap_or_default(),
                    binary.env.unwrap_or_default().into_iter().collect(),
                )
            },
        );
        let command = path.or_else(|| worktree.which("locus-lsp")).ok_or_else(|| {
            "Cannot find locus-lsp. Install it with `cargo install --path crates/locus-lsp --locked` \
             from the Locus repository, or set lsp.locus-lsp.binary.path in Zed settings."
                .to_owned()
        })?;

        Ok(zed::Command { command, args, env })
    }
}

zed::register_extension!(LocusExtension);
