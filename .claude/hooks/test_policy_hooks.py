import unittest

from test_helpers import load_hook_module

block_direct_git_ops = load_hook_module("block-direct-git-ops")


class PolicyHooksTest(unittest.TestCase):
    def test_block_direct_git_ops_uses_constant_messages(self) -> None:
        should_block, message = block_direct_git_ops.check_command("git commit -m test")
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

        should_block, message = block_direct_git_ops.check_command("git add src/lib.rs")
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

        should_block, message = block_direct_git_ops.check_command(
            "git branch -d old-branch"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    def test_block_direct_git_ops_rejects_nested_shell_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "bash -lc 'git add src/lib.rs'"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_direct_git_ops_rejects_nested_shell_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            'sh -c "git commit -m test"'
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_rejects_find_exec_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "find . -exec git commit -m test \\;"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_rejects_python_os_system_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "python3 -c \"import os; os.system('git commit -m test')\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_rejects_python_subprocess_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "python3 -c \"import subprocess; subprocess.run(['git', 'add', 'src/lib.rs'])\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_direct_git_ops_rejects_absolute_path_python_subprocess_git_commit(
        self,
    ) -> None:
        # Absolute path binary in list: subprocess.run(["/usr/bin/git", "commit", ...])
        should_block, message = block_direct_git_ops.check_command(
            "python3 -c \"import subprocess; subprocess.run(['/usr/bin/git', 'commit', '-m', 'msg'])\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_rejects_absolute_path_python_subprocess_git_add(
        self,
    ) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "python3 -c \"import subprocess; subprocess.run(['/usr/bin/git', 'add', 'src/lib.rs'])\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_direct_git_ops_does_not_block_absolute_path_python_subprocess_git_status(
        self,
    ) -> None:
        # git status via absolute path must not be blocked.
        should_block, _ = block_direct_git_ops.check_command(
            "python3 -c \"import subprocess; subprocess.run(['/usr/bin/git', 'status'])\""
        )
        self.assertFalse(should_block)

    def test_block_direct_git_ops_does_not_block_echoed_git_text(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            'bash -lc "echo git add src/lib.rs"'
        )
        self.assertFalse(should_block)
        self.assertEqual(message, "")

    def test_block_direct_git_ops_does_not_crash_on_shell_flag_without_argument(
        self,
    ) -> None:
        # "bash -c" with no trailing command must not raise IndexError
        should_block, message = block_direct_git_ops.check_command("bash -c")
        self.assertFalse(should_block)
        self.assertEqual(message, "")

    def test_block_direct_git_ops_rejects_direct_and_and_chained_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git status && git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_rejects_direct_and_and_chained_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git diff HEAD || git add src/lib.rs"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_direct_git_ops_rejects_newline_chained_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git status\ngit commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_rejects_newline_chained_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git status\ngit add src/lib.rs"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_direct_git_ops_rejects_background_chained_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git status & git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_rejects_backtick_command_substitution_commit(
        self,
    ) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "echo `git commit -m test`"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_rejects_backtick_command_substitution_add(
        self,
    ) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "echo `git add src/lib.rs`"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_direct_git_ops_rejects_dollar_paren_command_substitution_commit(
        self,
    ) -> None:
        should_block, message = block_direct_git_ops.check_command(
            'printf "%s" "$(git commit -m test)"'
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_direct_git_ops_allows_chained_read_only_git_commands(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git status && git diff HEAD"
        )
        self.assertFalse(should_block)
        self.assertEqual(message, "")

    def test_block_direct_git_ops_does_not_block_read_only_git_commands(self) -> None:
        for cmd in ["git status", "git diff HEAD", "git log --oneline"]:
            should_block, _ = block_direct_git_ops.check_command(cmd)
            self.assertFalse(should_block, f"should not block: {cmd}")

    def test_block_direct_git_ops_allows_git_branch_create_and_rename(self) -> None:
        for cmd in [
            "git branch feature/new-api",
            "git branch -m feature/old feature/new",
            "git branch -M main trunk",
        ]:
            should_block, message = block_direct_git_ops.check_command(cmd)
            self.assertFalse(should_block, f"should not block: {cmd}")
            self.assertEqual(message, "")

    def test_block_direct_git_ops_allows_exact_wrapper_paths(self) -> None:
        for cmd in [
            "cargo make add-all",
            "cargo make add-pending-paths",
            "cargo make track-add-paths",
            "cargo make commit-pending-message",
            "cargo make track-commit-message",
            "cargo make note-pending",
            "cargo make track-note",
        ]:
            should_block, message = block_direct_git_ops.check_command(cmd)
            self.assertFalse(should_block, f"should not block: {cmd}")
            self.assertEqual(message, "")

    # --- git push block ---

    def test_block_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git push origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_git_push_force(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git push --force origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_git_push_bare(self) -> None:
        should_block, message = block_direct_git_ops.check_command("git push")
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_git_push_chained(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git status && git push origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    # --- git option-argument parsing (e.g. git -C dir add) ---

    def test_block_git_with_dash_C_option_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git -C /some/dir add src/lib.rs"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_git_with_dash_C_option_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git -C /some/dir commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_git_with_dash_C_option_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git -C /some/dir push origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_git_with_config_option_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git -c user.name=test add src/lib.rs"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_git_with_git_dir_option_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git --git-dir=.git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_git_with_work_tree_option_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git --work-tree /tmp add ."
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_allow_git_with_dash_C_option_status(self) -> None:
        should_block, _ = block_direct_git_ops.check_command("git -C /some/dir status")
        self.assertFalse(should_block)

    def test_block_git_branch_delete_with_dash_C(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git -C /some/dir branch -D old"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    # --- env option parsing bypass prevention ---

    def test_block_env_with_options_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "env -i git add src/lib.rs"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_env_with_unset_option_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "env -u HOME git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_env_with_chdir_option_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "env -C /tmp git push origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_env_with_double_dash_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "env -- git add src/lib.rs"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_env_with_var_and_option_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "env FOO=bar -i git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_absolute_env_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "/usr/bin/env git add src/lib.rs"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_env_empty_value_git_add(self) -> None:
        """VAR= (empty value) must not bypass the block."""
        should_block, message = block_direct_git_ops.check_command("VAR= git add .")
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_env_cmd_empty_value_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command("env FOO= git add .")
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_find_exec_env_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "find . -exec env git add . \\;"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    # --- Python subprocess git push detection ---

    def test_block_python_os_system_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "python3 -c \"import os; os.system('git push origin main')\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_python_subprocess_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "python3 -c \"import subprocess; subprocess.run(['git', 'push'])\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    # --- False positive prevention ---

    def test_allow_echo_exec_git_add(self) -> None:
        """echo -exec git add is not a real launcher — must not be blocked."""
        should_block, _ = block_direct_git_ops.check_command("echo -exec git add")
        self.assertFalse(should_block)

    def test_allow_echo_xargs_git_push(self) -> None:
        should_block, _ = block_direct_git_ops.check_command("echo xargs git push")
        self.assertFalse(should_block)

    # --- xargs with options ---

    def test_block_xargs_with_null_option_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command("xargs -0 git push")
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_xargs_with_env_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "xargs -0 env FOO=bar git add ."
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    # --- Python git branch -d ---

    def test_block_python_os_system_git_branch_delete(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "python3 -c \"import os; os.system('git branch -d old')\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    def test_block_python_os_system_git_branch_delete_D(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "python3 -c \"import os; os.system('git branch -D old')\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    # --- Bug fix: Python list-form without whitespace ---

    def test_block_python_list_form_no_whitespace_commit(self) -> None:
        """Regression: "git","commit" (no space) must not crash with IndexError."""
        should_block, message = block_direct_git_ops.check_command(
            """python3 -c 'import subprocess; subprocess.run(["git","commit","-m","test"])'"""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_python_list_form_no_whitespace_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            """python3 -c 'import subprocess; subprocess.run(["git","push"])'"""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    # --- Bug fix: xargs options with arguments ---

    def test_block_xargs_n1_git_commit(self) -> None:
        """Regression: xargs -n 1 git commit must be blocked (-n takes an argument)."""
        should_block, message = block_direct_git_ops.check_command(
            "xargs -n 1 git commit"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_xargs_I_replacement_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "xargs -I {} git push"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    # --- Bug fix: Python list-form branch delete ---

    def test_block_python_list_form_branch_delete(self) -> None:
        """Regression: subprocess.run(["git", "branch", "-D", "old"]) must be blocked."""
        should_block, message = block_direct_git_ops.check_command(
            """python3 -c 'subprocess.run(["git", "branch", "-D", "old"])'"""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    def test_block_python_list_form_branch_delete_lowercase(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            """python3 -c 'subprocess.run(["git", "branch", "-d", "old"])'"""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    def test_block_python_list_form_abspath_branch_delete(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            """python3 -c 'subprocess.run(["/usr/bin/git", "branch", "--delete", "old"])'"""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    # --- Round 4 fixes: python -W/-X, xargs -a/-d, env -iC, git --exec-path ---

    def test_block_python_W_flag_with_arg_git_commit(self) -> None:
        """python3 -W ignore -c must still detect nested git commit."""
        should_block, message = block_direct_git_ops.check_command(
            "python3 -W ignore -c \"import os; os.system('git commit -m test')\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_python_X_flag_with_arg_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "python3 -X dev -c \"import os; os.system('git push')\""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_xargs_a_option_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "xargs -a input git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_xargs_d_option_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            r"xargs -d '\n' git push"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_xargs_arg_file_long_option_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "xargs --arg-file input git add ."
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_env_combined_short_flag_iC_git_push(self) -> None:
        """env -iC /tmp git push: -iC is -i + -C (chdir takes argument)."""
        should_block, message = block_direct_git_ops.check_command(
            "env -iC /tmp git push"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_git_exec_path_separate_arg_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "git --exec-path /tmp push origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_git_exec_path_equals_push(self) -> None:
        """git --exec-path=/tmp push: equals form is handled by startswith('-') fallback."""
        should_block, message = block_direct_git_ops.check_command(
            "git --exec-path=/tmp push origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    # --- Round 5 fixes: command launchers, xargs/find branch delete ---

    def test_block_nohup_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "nohup git push origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_nice_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "nice -n 10 git add ."
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_timeout_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "timeout 5 git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_command_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "command git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_stdbuf_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "stdbuf -o0 git push"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_xargs_git_branch_delete(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "xargs -0 git branch -D old"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    def test_block_xargs_n1_git_branch_delete(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "xargs -n 1 git branch -d old"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    def test_block_find_exec_git_branch_delete(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            r"find . -exec git branch --delete old \;"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    def test_block_find_exec_env_git_branch_delete(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            r"find . -exec env FOO=bar git branch -D old \;"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE)

    # --- Round 6 fixes: shell options before -c, python -c'code', time/exec ---

    def test_block_bash_norc_c_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "bash --norc -c 'git commit -m test'"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_bash_O_extglob_c_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "bash -O extglob -c 'git push'"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_sh_e_c_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "sh -e -c 'git add .'"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_block_python_c_concatenated_push(self) -> None:
        """python3 -c'import os; os.system(\"git push\")' with no space after -c."""
        should_block, message = block_direct_git_ops.check_command(
            """python3 -c'import os; os.system("git push")'"""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_python_Wignore_c_git_commit(self) -> None:
        """python3 -Wignore -c (concatenated -W flag) must still detect -c."""
        should_block, message = block_direct_git_ops.check_command(
            """python3 -Wignore -c 'import os; os.system("git commit -m test")'"""
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_time_git_commit(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "time git commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_COMMIT_MESSAGE)

    def test_block_usr_bin_time_git_push(self) -> None:
        should_block, message = block_direct_git_ops.check_command(
            "/usr/bin/time git push"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_PUSH_MESSAGE)

    def test_block_exec_git_add(self) -> None:
        should_block, message = block_direct_git_ops.check_command("exec git add .")
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_ADD_MESSAGE)

    def test_allow_git_branch_create(self) -> None:
        """git branch without delete flags must not be blocked."""
        should_block, _ = block_direct_git_ops.check_command("git branch new-feature")
        self.assertFalse(should_block)

    def test_allow_timeout_git_status(self) -> None:
        """timeout with safe git commands must not be blocked."""
        should_block, _ = block_direct_git_ops.check_command("timeout 5 git status")
        self.assertFalse(should_block)

    def test_block_time_p_git_commit(self) -> None:
        """time -p git commit must be blocked (time's -p is a no-arg flag)."""
        should_block, _ = block_direct_git_ops.check_command(
            "time -p git commit -m test"
        )
        self.assertTrue(should_block)

    def test_block_shell_variable_git_commit(self) -> None:
        """$GIT commit must be blocked as a variable bypass."""
        should_block, message = block_direct_git_ops.check_command(
            "$GIT commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_VARIABLE_BYPASS_MESSAGE)

    def test_block_shell_variable_git_add(self) -> None:
        """${GIT} add must be blocked as a variable bypass."""
        should_block, message = block_direct_git_ops.check_command(
            "${GIT} add src/lib.rs"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_VARIABLE_BYPASS_MESSAGE)

    def test_block_command_substitution_which_git_push(self) -> None:
        """$(which git) push must be blocked as a variable bypass."""
        should_block, message = block_direct_git_ops.check_command(
            "$(which git) push origin main"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_VARIABLE_BYPASS_MESSAGE)

    def test_block_backtick_which_git_add(self) -> None:
        """`which git` add must be blocked as a variable bypass."""
        should_block, message = block_direct_git_ops.check_command("`which git` add .")
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_VARIABLE_BYPASS_MESSAGE)

    def test_block_env_variable_git_commit(self) -> None:
        """env $GIT_CMD commit must be blocked when expansion contains git."""
        should_block, message = block_direct_git_ops.check_command(
            "env $GIT_CMD commit -m test"
        )
        self.assertTrue(should_block)
        self.assertEqual(message, block_direct_git_ops.GIT_VARIABLE_BYPASS_MESSAGE)

    def test_allow_dollar_variable_without_git(self) -> None:
        """$FOO bar (no git in expansion) must not be blocked."""
        should_block, _ = block_direct_git_ops.check_command("$FOO bar")
        self.assertFalse(should_block)

    def test_allow_literal_git_status_not_affected(self) -> None:
        """Literal git status must still be allowed after variable bypass detection."""
        should_block, _ = block_direct_git_ops.check_command("git status")
        self.assertFalse(should_block)

    def test_block_direct_git_ops_exposes_constants_checked_by_orchestra_guardrails(
        self,
    ) -> None:
        # verify_orchestra_guardrails.py checks these markers as proof that the blocking
        # mechanism is in place. This test ensures those symbols still exist and contain
        # meaningful content so a rename does not silently break CI.
        self.assertIn("git add", block_direct_git_ops.GIT_ADD_MESSAGE.lower())
        self.assertIn("git commit", block_direct_git_ops.GIT_COMMIT_MESSAGE.lower())
        self.assertIn("git push", block_direct_git_ops.GIT_PUSH_MESSAGE.lower())
        self.assertIn(
            "git branch", block_direct_git_ops.GIT_BRANCH_DELETE_MESSAGE.lower()
        )


if __name__ == "__main__":
    unittest.main()
