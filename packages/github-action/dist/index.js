require('./sourcemap-register.js');/******/ (() => { // webpackBootstrap
/******/ 	"use strict";
/******/ 	var __webpack_modules__ = ({

/***/ 887:
/***/ (function(__unused_webpack_module, exports, __nccwpck_require__) {


var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", ({ value: true }));
const promises_1 = __nccwpck_require__(455);
const node_os_1 = __importDefault(__nccwpck_require__(161));
const node_path_1 = __importDefault(__nccwpck_require__(760));
const node_child_process_1 = __nccwpck_require__(421);
function getInput(name, fallback = "") {
    const key = `INPUT_${name.replace(/ /g, "_").toUpperCase()}`;
    const value = process.env[key];
    return value === undefined || value === "" ? fallback : value;
}
function getRequiredEnv(name) {
    const value = process.env[name];
    if (!value) {
        throw new Error(`${name} is required`);
    }
    return value;
}
function log(message) {
    process.stdout.write(`${message}\n`);
}
function fail(message) {
    process.stderr.write(`${message}\n`);
}
function run(command, args, options = {}) {
    const printable = [command, ...args].join(" ");
    log(`$ ${printable}`);
    const result = (0, node_child_process_1.spawnSync)(command, args, {
        stdio: "pipe",
        encoding: "utf8",
        ...options,
    });
    if (result.stdout) {
        process.stdout.write(result.stdout);
    }
    if (result.stderr) {
        process.stderr.write(result.stderr);
    }
    if (result.status !== 0) {
        throw new Error(`command failed: ${printable}\nstdout:\n${result.stdout.trimEnd()}\nstderr:\n${result.stderr.trimEnd()}`);
    }
    return result.stdout.trim();
}
async function githubRequest(token, requestPath) {
    const response = await fetch(`https://api.github.com${requestPath}`, {
        headers: {
            Accept: "application/vnd.github+json",
            Authorization: `Bearer ${token}`,
            "User-Agent": "pguilbert-canopy-action",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    });
    if (!response.ok) {
        throw new Error(`GitHub API request failed (${response.status}): ${requestPath}`);
    }
    return (await response.json());
}
async function listOpenPullRequests(token, repository) {
    const pulls = [];
    for (let page = 1;; page += 1) {
        const batch = await githubRequest(token, `/repos/${repository}/pulls?state=open&per_page=100&page=${page}`);
        pulls.push(...batch);
        if (batch.length < 100) {
            return pulls;
        }
    }
}
function unique(values) {
    return [...new Set(values)];
}
function impactedLabelsFromEvent(eventName, payload, openPrs, labelPrefix) {
    if (eventName === "push") {
        const pushedRef = payload.ref ?? "";
        const pushedBranch = pushedRef.replace(/^refs\/heads\//, "");
        return unique(openPrs
            .filter((pr) => pr.base.ref === pushedBranch)
            .flatMap((pr) => (pr.labels ?? []).map((label) => label.name))
            .filter((name) => typeof name === "string" && name.startsWith(labelPrefix))).sort();
    }
    if (payload.action === "labeled" || payload.action === "unlabeled") {
        const labelName = payload.label?.name;
        return typeof labelName === "string" && labelName.startsWith(labelPrefix) ? [labelName] : [];
    }
    const prLabels = (payload.pull_request?.labels ?? []).map((label) => label.name);
    return unique(prLabels.filter((name) => typeof name === "string" && name.startsWith(labelPrefix))).sort();
}
function selectPrsForLabel(openPrs, label) {
    return openPrs
        .filter((pr) => (pr.labels ?? []).some((candidate) => candidate.name === label))
        .sort((left, right) => left.number - right.number);
}
function resolveReleaseTarget() {
    const platforms = {
        linux: "unknown-linux-gnu",
        darwin: "apple-darwin",
    };
    const architectures = {
        x64: "x86_64",
        arm64: "aarch64",
    };
    const osTarget = platforms[process.platform];
    if (!osTarget) {
        throw new Error(`unsupported runner platform: ${process.platform}`);
    }
    const archTarget = architectures[process.arch];
    if (!archTarget) {
        throw new Error(`unsupported runner architecture: ${process.arch}`);
    }
    return `${archTarget}-${osTarget}`;
}
async function downloadFile(url, destination) {
    const response = await fetch(url, {
        headers: {
            "User-Agent": "pguilbert-canopy-action",
        },
        redirect: "follow",
    });
    if (!response.ok) {
        throw new Error(`failed to download ${url}: ${response.status}`);
    }
    const buffer = Buffer.from(await response.arrayBuffer());
    await (0, promises_1.writeFile)(destination, buffer);
}
async function ensureCanopyBinary(version) {
    const target = resolveReleaseTarget();
    const actionRepository = process.env.GITHUB_ACTION_REPOSITORY || "pguilbert/canopy";
    const archiveName = `canopy-${target}.tar.gz`;
    const releaseUrl = version === "latest"
        ? `https://github.com/${actionRepository}/releases/latest/download/${archiveName}`
        : `https://github.com/${actionRepository}/releases/download/${version}/${archiveName}`;
    const tempDir = await (0, promises_1.mkdtemp)(node_path_1.default.join(node_os_1.default.tmpdir(), "canopy-action-"));
    const archivePath = node_path_1.default.join(tempDir, archiveName);
    const binaryPath = node_path_1.default.join(tempDir, "canopy");
    log(`Downloading canopy from ${releaseUrl}`);
    await downloadFile(releaseUrl, archivePath);
    run("tar", ["-xzf", archivePath, "-C", tempDir]);
    await (0, promises_1.chmod)(binaryPath, 0o755);
    return binaryPath;
}
async function resolveCanopyBinary() {
    const configuredPath = getInput("canopy-path");
    if (configuredPath) {
        return node_path_1.default.resolve(configuredPath);
    }
    return ensureCanopyBinary(getInput("canopy-version", "latest"));
}
function validatePrGroup(prs, repository, label) {
    const foreignPrs = prs
        .filter((pr) => pr.head.repo.full_name !== repository)
        .map((pr) => pr.number);
    if (foreignPrs.length > 0) {
        throw new Error(`label ${label} is applied to fork PRs (${foreignPrs.join(", ")}); only same-repository PRs are supported`);
    }
    const bases = unique(prs.map((pr) => pr.base.ref));
    if (bases.length !== 1) {
        throw new Error(`label ${label} spans multiple base branches: ${bases.join(", ")}`);
    }
    return bases[0];
}
function deleteRemoteBranch(targetBranch) {
    log(`Deleting remote branch ${targetBranch}`);
    try {
        run("git", ["push", "origin", "--delete", targetBranch]);
    }
    catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (message.includes("remote ref does not exist") || message.includes("unable to delete")) {
            log(`Remote branch ${targetBranch} does not exist; skipping delete`);
            return;
        }
        throw error;
    }
}
function rebuildBranch(canopyBinary, label, labelPrefix, branchPrefix, repository, prs) {
    const suffix = label.slice(labelPrefix.length);
    const targetBranch = `${branchPrefix}${suffix}`;
    const baseRef = validatePrGroup(prs, repository, label);
    const headRefs = unique(prs.map((pr) => pr.head.ref));
    log(`Rebuilding ${targetBranch} from base ${baseRef} for label ${label}`);
    run(canopyBinary, [
        "branch",
        "--remote",
        "origin",
        "--push",
        "--force",
        "--base",
        baseRef,
        targetBranch,
        ...headRefs,
    ]);
}
async function main() {
    try {
        const token = getInput("github-token", process.env.GITHUB_TOKEN || "");
        if (!token) {
            throw new Error("github-token input is required");
        }
        const repository = getRequiredEnv("GITHUB_REPOSITORY");
        const labelPrefix = getInput("label-prefix", "canopy/");
        const branchPrefix = getInput("branch-prefix", "canopy-");
        const eventName = getRequiredEnv("GITHUB_EVENT_NAME");
        const eventPath = getRequiredEnv("GITHUB_EVENT_PATH");
        const payload = JSON.parse(await (0, promises_1.readFile)(eventPath, "utf8"));
        const openPrs = await listOpenPullRequests(token, repository);
        const impactedLabels = impactedLabelsFromEvent(eventName, payload, openPrs, labelPrefix);
        if (impactedLabels.length === 0) {
            log("No canopy labels were affected");
            return;
        }
        const canopyBinary = await resolveCanopyBinary();
        for (const label of impactedLabels) {
            const prs = selectPrsForLabel(openPrs, label);
            const suffix = label.slice(labelPrefix.length);
            const targetBranch = `${branchPrefix}${suffix}`;
            if (prs.length === 0) {
                deleteRemoteBranch(targetBranch);
                continue;
            }
            rebuildBranch(canopyBinary, label, labelPrefix, branchPrefix, repository, prs);
        }
    }
    catch (error) {
        fail(`error: ${error instanceof Error ? error.message : String(error)}`);
        process.exitCode = 1;
    }
}
void main();


/***/ }),

/***/ 421:
/***/ ((module) => {

module.exports = require("node:child_process");

/***/ }),

/***/ 455:
/***/ ((module) => {

module.exports = require("node:fs/promises");

/***/ }),

/***/ 161:
/***/ ((module) => {

module.exports = require("node:os");

/***/ }),

/***/ 760:
/***/ ((module) => {

module.exports = require("node:path");

/***/ })

/******/ 	});
/************************************************************************/
/******/ 	// The module cache
/******/ 	var __webpack_module_cache__ = {};
/******/ 	
/******/ 	// The require function
/******/ 	function __nccwpck_require__(moduleId) {
/******/ 		// Check if module is in cache
/******/ 		var cachedModule = __webpack_module_cache__[moduleId];
/******/ 		if (cachedModule !== undefined) {
/******/ 			return cachedModule.exports;
/******/ 		}
/******/ 		// Create a new module (and put it into the cache)
/******/ 		var module = __webpack_module_cache__[moduleId] = {
/******/ 			// no module.id needed
/******/ 			// no module.loaded needed
/******/ 			exports: {}
/******/ 		};
/******/ 	
/******/ 		// Execute the module function
/******/ 		var threw = true;
/******/ 		try {
/******/ 			__webpack_modules__[moduleId].call(module.exports, module, module.exports, __nccwpck_require__);
/******/ 			threw = false;
/******/ 		} finally {
/******/ 			if(threw) delete __webpack_module_cache__[moduleId];
/******/ 		}
/******/ 	
/******/ 		// Return the exports of the module
/******/ 		return module.exports;
/******/ 	}
/******/ 	
/************************************************************************/
/******/ 	/* webpack/runtime/compat */
/******/ 	
/******/ 	if (typeof __nccwpck_require__ !== 'undefined') __nccwpck_require__.ab = __dirname + "/";
/******/ 	
/************************************************************************/
/******/ 	
/******/ 	// startup
/******/ 	// Load entry module and return exports
/******/ 	// This entry module is referenced by other modules so it can't be inlined
/******/ 	var __webpack_exports__ = __nccwpck_require__(887);
/******/ 	module.exports = __webpack_exports__;
/******/ 	
/******/ })()
;
//# sourceMappingURL=index.js.map