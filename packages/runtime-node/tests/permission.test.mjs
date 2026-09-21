import assert from 'node:assert/strict';
import { Buffer, EventEmitter, isGranted, patternAllows, log, http, storage, file, time, crypto, custom, metado, path, PermissionDeniedError, ExecutionError, resetGrants } from '@metado/runtime';
import { __METADO_TEST__ } from '@metado/runtime/test';

resetGrants();

const denied = async (fn) => assert.rejects(fn, PermissionDeniedError);
const notAvailable = async (fn) => assert.rejects(fn, (e) => e instanceof ExecutionError && /not available in Node/.test(e.message));

// 形状：命名空间对象 + 方法函数（镜像 runtime-export-spec）
assert.equal(typeof http, 'object');
assert.equal(typeof http.get, 'function');
assert.equal(typeof http.post, 'function');
assert.equal(typeof storage.read, 'function');
assert.equal(typeof storage.write, 'function');
assert.equal(typeof file.read, 'function');
assert.equal(typeof file.stat, 'function');
assert.equal(typeof time.now, 'function');
assert.equal(typeof time.sleep, 'function');
assert.equal(typeof log.info, 'function');
assert.equal(typeof crypto.randomBytes, 'function');
assert.equal(typeof crypto.sha256, 'function');
assert.equal(typeof crypto.hmac, 'function');
assert.equal(typeof custom.dispatch, 'function');
assert.equal(typeof metado.custom, 'function');

// 未授权：异步形状 → rejected Promise（PermissionDenied）；同步形状 → 同步 throw
const t1 = await denied(() => http.get('https://example.com'));
const t2 = await denied(() => storage.read('k'));
const t3 = await denied(() => crypto.sha256(new Uint8Array(0)));
const t4 = await denied(() => file.stat('/etc/passwd'));
const t5 = await denied(() => custom.dispatch('alert', {}));
const t6 = await denied(() => metado.custom('alert', {}));
assert.throws(() => log.info('x'), PermissionDeniedError);
assert.throws(() => time.now(), PermissionDeniedError);
assert.throws(() => crypto.randomBytes(4), PermissionDeniedError);

// 错误层次：PermissionDeniedError instanceof ExecutionError（宿主可统一 catch）
const deniedErr = new PermissionDeniedError('http.get');
assert.equal(deniedErr instanceof ExecutionError, true);
assert.equal(deniedErr instanceof Error, true);
assert.equal(deniedErr.permission, 'http.get');

// 授权后：异步不再拒绝，stub 抛 "not available in Node"（ExecutionError）
__METADO_TEST__.grant('http.get', 'storage.read', 'crypto.sha256', 'custom.alert', 'log.info');
assert.equal(isGranted('http.get'), true);
const t7 = await notAvailable(() => http.get('https://example.com'));
const t8 = await notAvailable(() => storage.read('k'));
const t9 = await notAvailable(() => crypto.sha256(new Uint8Array(0)));
const t10 = await notAvailable(() => custom.dispatch('alert', {}));
const t11 = await notAvailable(() => metado.custom('alert', {}));
assert.throws(() => log.info('x'), (e) => e instanceof ExecutionError && /not available in Node/.test(e.message));

// 部分授权矩阵：未授权项仍拒
const t12 = await denied(() => http.post('https://example.com'));

// 权限匹配（与引擎段级语义一致）：细粒度 grant 覆盖父方法；尾段 * 贪心
resetGrants();
assert.equal(isGranted('http.get'), false);
__METADO_TEST__.grant('http.get.api.*');
assert.equal(patternAllows('http.get.api.*', 'http.get'), true);
assert.equal(patternAllows('http.get.api.*', 'http.get.api.example.com'), true);
assert.equal(patternAllows('http.get.api.*', 'http.post'), false);
assert.equal(isGranted('http.get'), true);
assert.equal(isGranted('http.get.api.example.com'), true);
assert.equal(isGranted('http.post'), false);
resetGrants();

// shim：Buffer / path / events
assert.equal(Buffer.from('ok').toString(), 'ok');
assert.equal(path.basename('/a/b/c.js'), 'c.js');
const e = new EventEmitter();
let fired = false;
e.once('x', () => { fired = true; });
e.emit('x');
assert.equal(fired, true);

resetGrants();

const count = [t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12].length;
console.log(`permission.test.mjs: all assertions passed (${count} capability cases + shim + errors)`);