// The shared cases of spec/ and the interface XML.

import {
    INTERFACE, METHODS, classifyCaller, gnomeCapabilities, normalizeAppId,
} from '../lib/contract.js';
import {assert, equal, repoFile} from './assert.js';

export const tests = {
    'app.id cases'() {
        const {cases} = JSON.parse(repoFile('spec/appid-cases.json'));
        assert(cases.length > 0);
        for (const item of cases)
            equal(normalizeAppId(item.desktop_id, item.class), item.app, item.name);
    },
    'sandbox cases'() {
        const {cases} = JSON.parse(repoFile('spec/sandbox-cases.json'));
        assert(cases.length > 0);
        for (const item of cases) {
            const evidence = {pid: item.pid, flatpakInfo: item.flatpak_info, cgroup: item.cgroup};
            equal(classifyCaller(evidence), item.verdict, item.name);
        }
    },
    'shipped XML is the spec XML'() {
        equal(repoFile(`gnome-extension/${INTERFACE}.xml`),
            repoFile(`spec/${INTERFACE}.xml`));
    },
    'methods match the XML'() {
        const xml = repoFile(`spec/${INTERFACE}.xml`);
        const methods = [...xml.matchAll(/<method name="(\w+)"/g)]
            .map(match => match[1])
            .filter(name => !name.startsWith('Get'));
        equal(methods, [...METHODS]);
    },
    'capabilities leave out workspace modes only'() {
        const capabilities = gnomeCapabilities();
        assert(!capabilities.includes('SetWorkspaceMode'));
        for (const method of METHODS.filter(name => name !== 'SetWorkspaceMode'))
            assert(capabilities.includes(method), method);
        assert(capabilities.includes('windows'));
    },
};
