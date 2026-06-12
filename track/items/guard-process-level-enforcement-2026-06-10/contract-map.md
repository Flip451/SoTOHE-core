<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
flowchart LR
classDef aggregate_root fill:#ede9fe,stroke:#4c1d95,stroke-width:2px
classDef app_service fill:#ecfdf5,stroke:#059669,stroke-width:2px
classDef command fill:#fff7ed,stroke:#c2410c,stroke-width:1px
classDef domain_service fill:#fee2e2,stroke:#991b1b,stroke-width:1px
classDef dto fill:#f8fafc,stroke:#64748b,stroke-width:1px
classDef entity fill:#dbeafe,stroke:#1e40af,stroke-width:2px
classDef error_type fill:#fef2f2,stroke:#b91c1c,stroke-width:1px,stroke-dasharray:4 2
classDef factory fill:#e0f2fe,stroke:#0369a1,stroke-width:1px
classDef free_function fill:#f5f3ff,stroke:#7c3aed,stroke-width:1px
classDef function_node fill:#f5f3ff,stroke:#a78bfa,stroke-width:1px
classDef interactor fill:#f0fdfa,stroke:#0d9488,stroke-width:1px
classDef method_node fill:#f8fafc,stroke:#cbd5e1,stroke-width:1px
classDef query fill:#f0f9ff,stroke:#0369a1,stroke-width:1px
classDef secondary_adapter fill:#fafaf9,stroke:#57534e,stroke-width:1px
classDef secondary_port fill:#fafaf9,stroke:#78716c,stroke-width:1px,stroke-dasharray:4 2
classDef specification fill:#fdf4ff,stroke:#6b21a8,stroke-width:1px
classDef specification_port fill:#fdf4ff,stroke:#9333ea,stroke-width:1px,stroke-dasharray:4 2
classDef typestate_overlay stroke:#dc2626,stroke-width:3px
classDef use_case fill:#ecfeff,stroke:#0e7490,stroke-width:1px
classDef use_case_function fill:#eef2ff,stroke:#4338ca,stroke-width:1px
classDef value_object fill:#d1fae5,stroke:#065f46,stroke-width:1px
classDef variant_node fill:#fafaf9,stroke:#d6d3d1,stroke-width:1px
subgraph domain["domain"]
  direction TB
  subgraph domain_domain_module_guard["domain::guard"]
    direction TB
  subgraph T26_domain_domain_GuardVerdict["guard::verdict::GuardVerdict"]
    direction TB
    T26_domain_domain_GuardVerdict__self[GuardVerdict]
  end
  subgraph T24_domain_domain_ParseError["guard::verdict::ParseError"]
    direction TB
    T24_domain_domain_ParseError__self[ParseError]
    T24_domain_domain_ParseError_NestingDepthExceeded[NestingDepthExceeded]
    T24_domain_domain_ParseError_UnmatchedQuote[UnmatchedQuote]
  end
  subgraph T27_domain_domain_SimpleCommand["guard::types::SimpleCommand"]
    direction TB
    T27_domain_domain_SimpleCommand__self[SimpleCommand]
  end
  subgraph R25_domain_domain_ShellParser["guard::port::ShellParser"]
    direction TB
    R25_domain_domain_ShellParser__self[ShellParser]
    R25_domain_domain_ShellParser_split_shell([split_shell])
  end
  F57_domain_domain_domain__guard__policy__block_on_parse_error[[block_on_parse_error]]
  F51_domain_domain_domain__guard__policy__check_commands[[check_commands]]
  end
  subgraph domain_domain_module_hook["domain::hook"]
    direction TB
  subgraph T25_domain_domain_HookContext["hook::types::HookContext"]
    direction TB
    T25_domain_domain_HookContext__self[HookContext]
  end
  subgraph T23_domain_domain_HookError["hook::error::HookError"]
    direction TB
    T23_domain_domain_HookError__self[HookError]
    T23_domain_domain_HookError_Input[Input]
    T23_domain_domain_HookError_Guard[Guard]
    T23_domain_domain_HookError_Unsupported[Unsupported]
  end
  subgraph T23_domain_domain_HookInput["hook::types::HookInput"]
    direction TB
    T23_domain_domain_HookInput__self[HookInput]
  end
  subgraph T22_domain_domain_HookName["hook::types::HookName"]
    direction TB
    T22_domain_domain_HookName__self[HookName]
    T22_domain_domain_HookName_BlockDirectGitOps[BlockDirectGitOps]
    T22_domain_domain_HookName_BlockTestFileDeletion[BlockTestFileDeletion]
    T22_domain_domain_HookName_GitRefUpdate[GitRefUpdate]
    T22_domain_domain_HookName_GitPrePush[GitPrePush]
  end
  subgraph T25_domain_domain_HookVerdict["hook::verdict::HookVerdict"]
    direction TB
    T25_domain_domain_HookVerdict__self[HookVerdict]
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_hook["usecase::hook"]
    direction TB
  subgraph T33_usecase_usecase_GitPrePushHandler["hook::GitPrePushHandler"]
    direction TB
    T33_usecase_usecase_GitPrePushHandler__self[GitPrePushHandler]
  end
  subgraph T35_usecase_usecase_GitRefUpdateHandler["hook::GitRefUpdateHandler"]
    direction TB
    T35_usecase_usecase_GitRefUpdateHandler__self[GitRefUpdateHandler]
  end
  subgraph T32_usecase_usecase_GuardHookHandler["hook::GuardHookHandler"]
    direction TB
    T32_usecase_usecase_GuardHookHandler__self[GuardHookHandler]
  end
  subgraph T37_usecase_usecase_HooksPathSetupHandler["hook::HooksPathSetupHandler"]
    direction TB
    T37_usecase_usecase_HooksPathSetupHandler__self[HooksPathSetupHandler]
  end
  subgraph T44_usecase_usecase_TestFileDeletionGuardHandler["hook::TestFileDeletionGuardHandler"]
    direction TB
    T44_usecase_usecase_TestFileDeletionGuardHandler__self[TestFileDeletionGuardHandler]
  end
  subgraph R27_usecase_usecase_HookHandler["hook::HookHandler"]
    direction TB
    R27_usecase_usecase_HookHandler__self[HookHandler]
    R27_usecase_usecase_HookHandler_handle([handle])
  end
  F39_usecase_usecase_usecase__hook__dispatch[[dispatch]]
  end
  subgraph usecase_usecase_module_hook_dispatch["usecase::hook_dispatch"]
    direction TB
  subgraph T35_usecase_usecase_HookDispatchCommand["hook_dispatch::HookDispatchCommand"]
    direction TB
    T35_usecase_usecase_HookDispatchCommand__self[HookDispatchCommand]
  end
  subgraph T33_usecase_usecase_HookDispatchError["hook_dispatch::HookDispatchError"]
    direction TB
    T33_usecase_usecase_HookDispatchError__self[HookDispatchError]
    T33_usecase_usecase_HookDispatchError_UnknownHookName[UnknownHookName]
    T33_usecase_usecase_HookDispatchError_HandlerFailed[HandlerFailed]
  end
  subgraph T38_usecase_usecase_HookDispatchInteractor["hook_dispatch::HookDispatchInteractor"]
    direction TB
    T38_usecase_usecase_HookDispatchInteractor__self[HookDispatchInteractor]
    T38_usecase_usecase_HookDispatchInteractor_new([new])
  end
  subgraph T35_usecase_usecase_HookVerdictDecision["hook_dispatch::HookVerdictDecision"]
    direction TB
    T35_usecase_usecase_HookVerdictDecision__self[HookVerdictDecision]
    T35_usecase_usecase_HookVerdictDecision_Allow[Allow]
    T35_usecase_usecase_HookVerdictDecision_Block[Block]
  end
  subgraph T33_usecase_usecase_HookVerdictOutput["hook_dispatch::HookVerdictOutput"]
    direction TB
    T33_usecase_usecase_HookVerdictOutput__self[HookVerdictOutput]
  end
  subgraph R35_usecase_usecase_HookDispatchService["hook_dispatch::HookDispatchService"]
    direction TB
    R35_usecase_usecase_HookDispatchService__self[HookDispatchService]
    R35_usecase_usecase_HookDispatchService_dispatch([dispatch])
  end
  subgraph R35_usecase_usecase_HookShellParserPort["hook_dispatch::HookShellParserPort"]
    direction TB
    R35_usecase_usecase_HookShellParserPort__self[HookShellParserPort]
    R35_usecase_usecase_HookShellParserPort_split_shell([split_shell])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_git_cli["infrastructure::git_cli"]
    direction TB
  subgraph T38_infrastructure_infrastructure_GitError["git_cli::GitError"]
    direction TB
    T38_infrastructure_infrastructure_GitError__self[GitError]
    T38_infrastructure_infrastructure_GitError_CurrentDir[CurrentDir]
    T38_infrastructure_infrastructure_GitError_Spawn[Spawn]
    T38_infrastructure_infrastructure_GitError_CommandFailed[CommandFailed]
    T38_infrastructure_infrastructure_GitError_EmptyRepoRoot[EmptyRepoRoot]
  end
  subgraph T43_infrastructure_infrastructure_SystemGitRepo["git_cli::SystemGitRepo"]
    direction TB
    T43_infrastructure_infrastructure_SystemGitRepo__self[SystemGitRepo]
    T43_infrastructure_infrastructure_SystemGitRepo_discover([discover])
    T43_infrastructure_infrastructure_SystemGitRepo_discover_from([discover_from])
  end
  subgraph T47_infrastructure_infrastructure_TrackBranchRecord["git_cli::TrackBranchRecord"]
    direction TB
    T47_infrastructure_infrastructure_TrackBranchRecord__self[TrackBranchRecord]
  end
  F82_infrastructure_infrastructure_infrastructure__git_cli__collect_track_branch_claims[[collect_track_branch_claims]]
  F81_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch[[load_explicit_track_branch]]
  F96_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch_from_items_dir[[load_explicit_track_branch_from_items_dir]]
  F72_infrastructure_infrastructure_infrastructure__git_cli__resolve_repo_path[[resolve_repo_path]]
  end
  subgraph infrastructure_infrastructure_module_verify["infrastructure::verify"]
    direction TB
  F72_infrastructure_infrastructure_infrastructure__verify__hooks_path__verify[[verify]]
  end
end
R25_domain_domain_ShellParser_split_shell --> T24_domain_domain_ParseError__self
R25_domain_domain_ShellParser_split_shell --> T27_domain_domain_SimpleCommand__self
F57_domain_domain_domain__guard__policy__block_on_parse_error --o T24_domain_domain_ParseError__self
F57_domain_domain_domain__guard__policy__block_on_parse_error --> T26_domain_domain_GuardVerdict__self
F51_domain_domain_domain__guard__policy__check_commands --o T27_domain_domain_SimpleCommand__self
F51_domain_domain_domain__guard__policy__check_commands --> T26_domain_domain_GuardVerdict__self
T23_domain_domain_HookError_Guard --o T24_domain_domain_ParseError__self
T23_domain_domain_HookError_Unsupported --o T22_domain_domain_HookName__self
R27_usecase_usecase_HookHandler_handle --o T25_domain_domain_HookContext__self
R27_usecase_usecase_HookHandler_handle --o T23_domain_domain_HookInput__self
R27_usecase_usecase_HookHandler_handle --> T23_domain_domain_HookError__self
R27_usecase_usecase_HookHandler_handle --> T25_domain_domain_HookVerdict__self
F39_usecase_usecase_usecase__hook__dispatch --o T22_domain_domain_HookName__self
F39_usecase_usecase_usecase__hook__dispatch --o T25_domain_domain_HookContext__self
F39_usecase_usecase_usecase__hook__dispatch --o T23_domain_domain_HookInput__self
F39_usecase_usecase_usecase__hook__dispatch --> T23_domain_domain_HookError__self
F39_usecase_usecase_usecase__hook__dispatch --> T25_domain_domain_HookVerdict__self
T38_usecase_usecase_HookDispatchInteractor_new --> T38_usecase_usecase_HookDispatchInteractor__self
T33_usecase_usecase_HookVerdictOutput__self --o|decision| T35_usecase_usecase_HookVerdictDecision__self
R35_usecase_usecase_HookDispatchService_dispatch --o T35_usecase_usecase_HookDispatchCommand__self
R35_usecase_usecase_HookDispatchService_dispatch --> T33_usecase_usecase_HookDispatchError__self
R35_usecase_usecase_HookDispatchService_dispatch --> T33_usecase_usecase_HookVerdictOutput__self
R35_usecase_usecase_HookShellParserPort_split_shell --> T24_domain_domain_ParseError__self
R35_usecase_usecase_HookShellParserPort_split_shell --> T27_domain_domain_SimpleCommand__self
T38_usecase_usecase_HookDispatchInteractor__self -.impl.-> R35_usecase_usecase_HookDispatchService__self
T35_usecase_usecase_GitRefUpdateHandler__self -.impl.-> R27_usecase_usecase_HookHandler__self
T33_usecase_usecase_GitPrePushHandler__self -.impl.-> R27_usecase_usecase_HookHandler__self
T32_usecase_usecase_GuardHookHandler__self -.impl.-> R27_usecase_usecase_HookHandler__self
T37_usecase_usecase_HooksPathSetupHandler__self -.impl.-> R27_usecase_usecase_HookHandler__self
T44_usecase_usecase_TestFileDeletionGuardHandler__self -.impl.-> R27_usecase_usecase_HookHandler__self
T43_infrastructure_infrastructure_SystemGitRepo_discover --> T38_infrastructure_infrastructure_GitError__self
T43_infrastructure_infrastructure_SystemGitRepo_discover --> T43_infrastructure_infrastructure_SystemGitRepo__self
T43_infrastructure_infrastructure_SystemGitRepo_discover_from --> T38_infrastructure_infrastructure_GitError__self
T43_infrastructure_infrastructure_SystemGitRepo_discover_from --> T43_infrastructure_infrastructure_SystemGitRepo__self
F82_infrastructure_infrastructure_infrastructure__git_cli__collect_track_branch_claims --> T47_infrastructure_infrastructure_TrackBranchRecord__self
F81_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch --> T47_infrastructure_infrastructure_TrackBranchRecord__self
F96_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch_from_items_dir --> T47_infrastructure_infrastructure_TrackBranchRecord__self
class T26_domain_domain_GuardVerdict__self value_object
class T24_domain_domain_ParseError_NestingDepthExceeded variant_node
class T24_domain_domain_ParseError_UnmatchedQuote variant_node
class T24_domain_domain_ParseError__self error_type
class T27_domain_domain_SimpleCommand__self value_object
class R25_domain_domain_ShellParser_split_shell method_node
class R25_domain_domain_ShellParser__self secondary_port
class F57_domain_domain_domain__guard__policy__block_on_parse_error free_function
class F57_domain_domain_domain__guard__policy__block_on_parse_error function_node
class F51_domain_domain_domain__guard__policy__check_commands free_function
class F51_domain_domain_domain__guard__policy__check_commands function_node
class T25_domain_domain_HookContext__self value_object
class T23_domain_domain_HookError_Input variant_node
class T23_domain_domain_HookError_Guard variant_node
class T23_domain_domain_HookError_Unsupported variant_node
class T23_domain_domain_HookError__self error_type
class T23_domain_domain_HookInput__self value_object
class T22_domain_domain_HookName_BlockDirectGitOps variant_node
class T22_domain_domain_HookName_BlockTestFileDeletion variant_node
class T22_domain_domain_HookName_GitRefUpdate variant_node
class T22_domain_domain_HookName_GitPrePush variant_node
class T22_domain_domain_HookName__self value_object
class T25_domain_domain_HookVerdict__self value_object
class T33_usecase_usecase_GitPrePushHandler__self interactor
class T35_usecase_usecase_GitRefUpdateHandler__self interactor
class T32_usecase_usecase_GuardHookHandler__self interactor
class T37_usecase_usecase_HooksPathSetupHandler__self interactor
class T44_usecase_usecase_TestFileDeletionGuardHandler__self interactor
class R27_usecase_usecase_HookHandler_handle method_node
class R27_usecase_usecase_HookHandler__self secondary_port
class F39_usecase_usecase_usecase__hook__dispatch free_function
class F39_usecase_usecase_usecase__hook__dispatch function_node
class T35_usecase_usecase_HookDispatchCommand__self command
class T33_usecase_usecase_HookDispatchError_UnknownHookName variant_node
class T33_usecase_usecase_HookDispatchError_HandlerFailed variant_node
class T33_usecase_usecase_HookDispatchError__self error_type
class T38_usecase_usecase_HookDispatchInteractor_new method_node
class T38_usecase_usecase_HookDispatchInteractor__self interactor
class T35_usecase_usecase_HookVerdictDecision_Allow variant_node
class T35_usecase_usecase_HookVerdictDecision_Block variant_node
class T35_usecase_usecase_HookVerdictDecision__self value_object
class T33_usecase_usecase_HookVerdictOutput__self dto
class R35_usecase_usecase_HookDispatchService_dispatch method_node
class R35_usecase_usecase_HookDispatchService__self app_service
class R35_usecase_usecase_HookShellParserPort_split_shell method_node
class R35_usecase_usecase_HookShellParserPort__self secondary_port
class T38_infrastructure_infrastructure_GitError_CurrentDir variant_node
class T38_infrastructure_infrastructure_GitError_Spawn variant_node
class T38_infrastructure_infrastructure_GitError_CommandFailed variant_node
class T38_infrastructure_infrastructure_GitError_EmptyRepoRoot variant_node
class T38_infrastructure_infrastructure_GitError__self error_type
class T43_infrastructure_infrastructure_SystemGitRepo_discover method_node
class T43_infrastructure_infrastructure_SystemGitRepo_discover_from method_node
class T43_infrastructure_infrastructure_SystemGitRepo__self secondary_adapter
class T47_infrastructure_infrastructure_TrackBranchRecord__self dto
class F82_infrastructure_infrastructure_infrastructure__git_cli__collect_track_branch_claims free_function
class F82_infrastructure_infrastructure_infrastructure__git_cli__collect_track_branch_claims function_node
class F81_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch free_function
class F81_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch function_node
class F96_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch_from_items_dir free_function
class F96_infrastructure_infrastructure_infrastructure__git_cli__load_explicit_track_branch_from_items_dir function_node
class F72_infrastructure_infrastructure_infrastructure__git_cli__resolve_repo_path free_function
class F72_infrastructure_infrastructure_infrastructure__git_cli__resolve_repo_path function_node
class F72_infrastructure_infrastructure_infrastructure__verify__hooks_path__verify free_function
class F72_infrastructure_infrastructure_infrastructure__verify__hooks_path__verify function_node
```
