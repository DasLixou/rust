use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::builder::{Builder, Kind, RunConfig, ShouldRun, Step};
use crate::core::build_steps::llvm;
use crate::core::config::TargetSelection;
use crate::t;
use crate::utils::helpers::exe;

#[derive(Clone)]
pub struct TpdeResult {
    pub tpde_llvm_include: PathBuf,
    pub tpde_llvm_out: PathBuf,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Tpde {
    pub target: TargetSelection,
}

impl Step for Tpde {
    type Output = TpdeResult;

    const IS_HOST: bool = true;

    fn should_run(run: ShouldRun<'_>) -> ShouldRun<'_> {
        run.path("src/tpde").path("src/bootstrap/src/core/build_steps") // TODO: remove build_steps dependency here
    }

    fn make_run(run: RunConfig<'_>) {
        run.builder.ensure(Tpde { target: run.target });
    }

    fn run(self, builder: &Builder<'_>) -> TpdeResult {
        let _ = builder.ensure(llvm::Llvm { target: self.target });

        let target = self.target;

        builder.config.update_submodule("src/tpde"); // TODO: this doesn't seem to work :/

        let tpde_dir = builder.src.join("src/tpde");
        let tpde_llvm_include = tpde_dir.join("tpde-llvm/include");

        let out_dir = builder.tpde_out(target);
        t!(fs::create_dir_all(&out_dir));

        let _guard = builder.msg_unstaged(Kind::Build, "TPDE", target);

        if builder.config.dry_run() {
            return TpdeResult { tpde_llvm_include, tpde_llvm_out: out_dir };
        }

        let mut cfg = cmake::Config::new(&tpde_dir);
        let ldflags = llvm::LdFlags::default();

        let build_llvm_config = if let Some(build_llvm_config) = builder
            .config
            .target_config
            .get(&builder.config.host_target)
            .and_then(|config| config.llvm_config.clone())
        {
            build_llvm_config
        } else {
            let mut llvm_config_ret_dir = builder.llvm_out(builder.config.host_target);
            llvm_config_ret_dir.push("bin");
            llvm_config_ret_dir.join(exe("llvm-config", builder.config.host_target))
        };

        cfg.out_dir(&out_dir)
            .profile("RelWithDebInfo") // TODO: change this
            .define("LLVM_INCLUDE_DIRS", builder.llvm_out(target).join("include"))
            .define("TPDE_INCLUDE_TESTS", "OFF")
            .define("TPDE_CLANG", "/usr/lib/llvm-19/bin/clang")
            .define(
                "RUST_LLVM_LIB",
                output(Command::new(&build_llvm_config).arg("--libfiles")).lines().next().unwrap(),
            );
        llvm::configure_cmake(builder, target, &mut cfg, true, ldflags, &[]);

        cfg.build();

        TpdeResult { tpde_llvm_include, tpde_llvm_out: out_dir }
    }
}

fn output(cmd: &mut Command) -> String {
    let output = match cmd.stderr(Stdio::inherit()).output() {
        Ok(status) => status,
        Err(e) => {
            println!("\n\nfailed to execute command: {cmd:?}\nerror: {e}\n\n");
            std::process::exit(1);
        }
    };
    if !output.status.success() {
        panic!(
            "command did not execute successfully: {:?}\n\
             expected success, got: {}",
            cmd, output.status
        );
    }
    String::from_utf8(output.stdout).unwrap()
}
