/** Domain enums aligned with `src/domain/*.rs` (shared by UI and IPC hooks). */

/** @enum {'AutoDiscovered' | 'UserAdded'} */
export const GameSource = {
  AutoDiscovered: "AutoDiscovered",
  UserAdded: "UserAdded",
};

/** @enum {'ActiveDirectory' | 'Vault'} */
export const SaveOrigin = {
  ActiveDirectory: "ActiveDirectory",
  Vault: "Vault",
};

/** @enum {'verified' | 'corrupted' | 'unchecked'} */
export const IntegrityStatus = {
  Verified: "verified",
  Corrupted: "corrupted",
  Unchecked: "unchecked",
};

/** @enum {'SourceNewer' | 'DestinationNewer' | 'Equal' | 'Unknown'} */
export const SaveFreshness = {
  SourceNewer: "SourceNewer",
  DestinationNewer: "DestinationNewer",
  Equal: "Equal",
  Unknown: "Unknown",
};

/** @enum {'KeepSource' | 'KeepDestination' | 'KeepBothRename' | 'CancelOperation'} */
export const ResolutionChoice = {
  KeepSource: "KeepSource",
  KeepDestination: "KeepDestination",
  KeepBothRename: "KeepBothRename",
  CancelOperation: "CancelOperation",
};
