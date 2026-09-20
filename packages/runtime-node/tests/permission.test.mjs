import assert from 'node:assert/strict';
import { Buffer, EventEmitter, isGranted, log, http, storage, crypto, custom, path, PermissionDeniedError, ExecutionError, resetGrants } from '@metado/runtime';
import { __METADO_TEST__ } from '@metado/runtime/test';

resetGrants();

const denied = async (fn) => assert.rejects(fn, PermissionDeniedError);
const notAvailable = async (fn) => assert.rejects(fn, (e) => e instanceof ExecutionError && /not available in Node/.test(e.message));

// 未授权：PermissionDenied 拒绝
const t1 = await denied(() => http.get.call());
const t2 = await denied(() => storage.read.call());
const t3 = await denied(() => log.info.call({ msg: 'x' }));
const t4 = await denied(() => crypto.sha256.call('a'));

// 授权后：不再拒绝，但 stub 仍抛 "not available in Node"（ExecutionError）
__METADO_TEST__.grant('http.get', 'storage.read', 'log.info', 'crypto.sha256');
assert.equal(isGranted('http.get'), true);
const t5 = await notAvailable(() => http.get.call());
const t6 = await notAvailable(() => storage.read.call());
const t7 = await notAvailable(() => log.info.call({ msg: 'x' }));
const t8 = await notAvailable(() => crypto.sha256.call('a'));

// 部分授权矩阵：未授权项仍拒
const t9 = await denied(() => http.post.call());

// 宿主自定义能力：任何访问即 ExecutionError
const t10 = await notAvailable(() => custom.alert.call());

// shim：Buffer / path / events
assert.equal(Buffer.from('ok').toString(), 'ok');
assert.equal(path.basename('/a/b/c.js'), 'c.js');
const e = new EventEmitter();
let fired = false;
e.once('x', () => { fired = true; });
e.emit('x');
assert.equal(fired, true);

resetGrants();

console.log(`permission.test.mjs: all assertions passed (${[t1,t2,t3,t4,t5,t6,t7,t8,t9,t10].length} capability cases + shim)`);