import { stub } from './stub.mjs';

export const httpGet = stub('http.get', 'http.get');
export const httpPost = stub('http.post', 'http.post');
export const httpGetApi = stub('http.get.api.*', 'http.get.api.*');

export const http = { get: httpGet, post: httpPost, api: httpGetApi };