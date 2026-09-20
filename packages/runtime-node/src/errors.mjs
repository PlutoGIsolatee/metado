/**
 * Error taxonomy for the Node dev shim (part of §12.4 execution report surface).
 */
export class MetadoError extends Error {}

export class ExecutionError extends MetadoError {}

export class PermissionDeniedError extends MetadoError {
  constructor(permission) {
    super(`permission denied: '${permission}' is not granted`);
    this.permission = permission;
  }
}