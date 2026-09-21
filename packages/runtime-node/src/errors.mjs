/**
 * Error taxonomy for the Node dev shim (mirrors runtime-export-spec §错误类型).
 * MetadoError → ExecutionError → PermissionDeniedError（宿主统一 catch ExecutionError）。
 */
export class MetadoError extends Error {}

export class ExecutionError extends MetadoError {}

export class PermissionDeniedError extends ExecutionError {
  constructor(permission) {
    super(`permission denied: '${permission}' is not granted`);
    this.permission = permission;
  }
}