#!/usr/bin/env python3
"""Pinned, reviewable upstream 0.3.107 / Extended integration candidate.

This script runs in an isolated CI checkout. It does not touch original refs.
Unhandled conflicts fail closed; no ours/theirs blanket conflict policy.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import sys

SOURCE="50b2d8bfd25ba2f885a9f060bfb93fcc4111cc34"
TARGET="6467995ea3b533b039bcd393e4e6a15e847f5464"
PREVIOUS="4f777ec3414a9c46d07ea58debb99c87fcf4e74f"
BRANCH="chore/pr120-reconcile-base-20261110"
MARKERS=re.compile(r"(?m)^<<<<<<< HEAD[^\n]*\n(.*?)^=======\n(.*?)^>>>>>>>[^\n]*\n",re.S|re.M)

def git(*args):
    p=subprocess.run(["git",*args],capture_output=True,text=True)
    if p.returncode: raise RuntimeError("git "+" ".join(args)+": "+p.stderr[-1200:])
    return p.stdout.strip()

def source_file(path):
    return git("show",SOURCE+":"+path)+"\n"

def checked_replace(data,old,new):
    if old not in data: raise RuntimeError("Missing expected code: "+old[:110])
    return data.replace(old,new,1)

def rust_function(data,signature):
    pos=data.index(signature)
    brace=data.index("{",pos)
    depth=0
    for i in range(brace,len(data)):
        if data[i]=="{": depth+=1
        elif data[i]=="}":
            depth-=1
            if depth==0: return data[pos:i+1]
    raise RuntimeError("Unclosed Rust method: "+signature)

def integrate_block(path,old,new,n):
    if path in {"package.json","package-lock.json","src-tauri/Cargo.toml","src-tauri/Cargo.lock","src-tauri/tauri.conf.json","docs/extended/upstream/source-baseline.json","website/src/data/release-dates.json"}:
        return new
    if path.startswith("docs/extended/") or path=="releases/extended/0.12.0.md":
        return old.rstrip("\n")+"\n"+new
    if path in {"playwright.config.ts","playwright.webkit.config.ts","src/main.ts"}:
        return old
    if path in {"src-tauri/src/agent_tools.rs","src-tauri/src/architect.rs","src-tauri/src/bots.rs"}:
        return old
    if path=="src-tauri/src/lib.rs":
        if n==1:
            return checked_replace(old,"slash_commands::snapshot(&provider, &cwd)?","slash_commands::snapshot(&provider, &cwd).await?")
        if n==2: return old
        if n==3:
            return "\n".join(line for line in old.split("\n") if "skills::skills_catalog" not in line and "skills::skills_discover," not in line)
    if path=="src-tauri/src/providers.rs":
        if n==1: return old
        if n==2:
            return checked_replace(old,"            if !config.reviewer_boundary && !codex_user_limits_threads",
              "            command.args(crate::skill_manager::codex_arguments(disabled));\n            if !config.reviewer_boundary && !codex_user_limits_threads")
        if n==3:
            return new+"\n    if config.reviewer_boundary {\n        sanitize_reviewer_environment(&mut command, &config.provider);\n    }\n"
    if path=="src-tauri/src/skills.rs":
        if n<=8: return old
        if n==9:
            a=old.find("#[tauri::command]\npub async fn skills_catalogs")
            b=old.find("#[tauri::command]\npub async fn skills_source_preview")
            if a<0 or b<a: raise RuntimeError("Failed to isolate obsolete catalog API")
            return old[:a]+old[b:]
        if n==10: return old.replace("mod catalog_tests;\n","")+"\n"+new
    if path=="src/preview.ts" and n==1:
        return checked_replace(old,"case 'skills_deactivate':","case 'skills_deactivate_all': case 'skills_deactivate':")
    if path=="src/preview-skills.ts":
        return old
    raise RuntimeError("Unreviewed conflict "+path+" #"+str(n))

def resolve_conflict_file(path):
    file=Path(path)
    data=file.read_text()
    seen=[0]
    def sub(match):
        seen[0]+=1
        return integrate_block(path,match.group(1),match.group(2),seen[0])
    data=MARKERS.sub(sub,data)
    if not seen[0] or "<<<<<<< " in data or ">>>>>>> " in data: raise RuntimeError("Unresolved "+path)
    file.write_text(data)
    print("RESOLVED",path,"blocks",seen[0])

def adapt_rules_library():
    f=Path("src/skills-library.ts")
    data=source_file(str(f))
    data=checked_replace(data,"kind: 'folder' | 'github' | 'skills';","kind: 'folder' | 'github' | 'zip' | 'skills';")
    item="  acknowledged: string | null; activations: SkillActivation[]; files: string[]; findings: SkillFinding[];"
    data=checked_replace(data,item,item+"\n  requiredSkills?: string[]; autoUpdate?: boolean | null; autoAvailable?: boolean; fileHashes?: Record<string,string>;")
    legacy="""// Types shared by Extended Customize Skills and its source preview.
export type CatalogSkill = { path: string; id: string; name: string; description: string; files: number; bytes: number; filePaths?: string[]; imported: string | null; conflicts: string[] };
export type CatalogSource = { repo: string; gitRef: string | null; commit: string | null; origin: 'manual' | 'discovered'; state: 'enabled' | 'disabled' | 'ignored'; private: boolean; pushedAt: string | null; scannedAt: number; skills: CatalogSkill[] };
export type SkillCatalogs = { sources: CatalogSource[]; discovery: { at: number; checked: number; registered: string[]; failures: string[]; note: string | null } | null };
export type CatalogImport = { items: SkillItem[]; imported: string[]; skipped: { path: string; name: string; reason: string }[] };

"""
    data=checked_replace(data,"export type SkillsLibraryHost = {",legacy+"export type SkillsLibraryHost = {")
    f.write_text(data)

def adapt_preview():
    f=Path("src/preview-skills.ts")
    data=f.read_text()
    if "import { LIBRARY_DIR }" not in data:
        data="import { LIBRARY_DIR } from './preview-skill-manager';\n"+data
    if "import type { CatalogImport," not in data:
        data=checked_replace(data,"import type { SkillActivation,","import type { CatalogImport, CatalogSkill, CatalogSource, SkillCatalogs, SkillActivation,")
    head=data.split("    const item: SkillItem = {",1)
    if len(head)!=2: raise RuntimeError("Missing preview item constructor")
    item=head[1].split("};",1)[0]
    if "folder:" not in item:
        data=checked_replace(data,"    const item: SkillItem = {","    const item: SkillItem = {\n      ...(kind === 'skill' ? { folder: LIBRARY_DIR + '/' + id } : {}),")
    extra="""    /** Shared with native Skills → Add skill → Bot library. */
    importSkills(source: string, skills: readonly { name: string; description: string }[]) {
      const kind = source.startsWith('/') ? 'folder' as const : 'skills' as const;
      const result: { imported: { id: string; name: string }[]; failures: { skill: string; reason: string }[] } = { imported: [], failures: [] };
      for (const skill of skills) {
        const taken = entries.get(slug(skill.name))?.item;
        if (taken && taken.source.location !== source) {
          result.failures.push({ skill: skill.name, reason: taken.id + ' is already in the library from ' + taken.source.location + '.' });
          continue;
        }
        const item = add('skill', skill.name, '# ' + skill.name + '\\n' + (skill.description || 'Added from ' + source + '.'), { kind, location: source, gitRef: null, commit: null });
        result.imported.push({ id: item.id, name: item.name });
      }
      return result;
    },
    deactivateAll(id: unknown) {
      const entry = find(id);
      const kept = entry.item.activations.flatMap(a => {
        const k = key(entry.item.id, a.harness, a.scope);
        const edited = a.surface !== 'prompt' && disk.has(k) && disk.get(k) !== a.sha256;
        if (!edited) disk.delete(k);
        return edited ? [cell(entry, a.harness, a.scope).surface] : [];
      });
      entry.item = { ...entry.item, activations: [] };
      return { items: list(), kept };
    },
"""
    # Git may already merge these upstream methods outside the conflict sections.
    if "importSkills(source:" not in data and "deactivateAll(id:" not in data:
        data=checked_replace(data,"    list,\n","    list,\n"+extra)
    elif "importSkills(source:" not in data or "deactivateAll(id:" not in data:
        raise RuntimeError("Partially merged upstream preview contracts require manual review")
    if "case 'skills_deactivate_all':" not in data:
        data=checked_replace(data,"case 'skills_remove':","case 'skills_deactivate_all': return this.deactivateAll(args.id);\n        case 'skills_remove':")
    f.write_text(data)

def adapt_rust():
    f=Path("src-tauri/src/skills.rs")
    data=f.read_text().replace("mod catalog;\n","").replace("mod catalog_tests;\n","")
    upstream=source_file(str(f))
    if "pub fn deactivate_all(&self" not in data:
        body=rust_function(upstream,"pub fn deactivate_all(&self")
        data=checked_replace(data,"    pub fn remove(&self, id: &str)","    "+body+"\n\n    pub fn remove(&self, id: &str)")
    if "pub async fn skills_deactivate_all" not in data:
        body=rust_function(upstream,"pub async fn skills_deactivate_all(")
        data=checked_replace(data,"#[tauri::command]\npub async fn skills_matrix(","#[tauri::command]\n"+body+"\n#[tauri::command]\npub async fn skills_matrix(")
    # The virtual-base conflict ends inside Extended's final preview command.
    # Restore the complete Extended command, then append the native upstream
    # test modules without splicing a module into an open function body.
    preview_anchor="#[tauri::command]\\npub async fn skills_github_preview_file("
    native_tests_anchor="#[cfg(test)]\\nmod folder_tests {"
    original=git("show",TARGET+":"+str(f))
    if data.count(preview_anchor)!=1 or original.count(preview_anchor)!=1 or native_tests_anchor not in upstream:
        raise RuntimeError("Unexpected Rust skills preview/test layout")
    data=(data[:data.index(preview_anchor)]
          +original[original.index(preview_anchor):].rstrip()
          +"\\n\\n"
          +upstream[upstream.index(native_tests_anchor):].rstrip()
          +"\\n")
    f.write_text(data)
    f=Path("src-tauri/src/slash_commands.rs")
    data=f.read_text()
    if "pub(crate) fn append_library_skills" not in data: raise RuntimeError("Lost Extended slash skill registration")
    helper="""/// Re-check native disabled-skill policy after adding Extended library commands.
pub(crate) async fn filter_disabled(provider: &str, catalog: Catalog, cwd: &Path) -> Result<Catalog, String> {
    skill_switches::for_path(provider, catalog, cwd).await
}

"""
    data=checked_replace(data,"pub(crate) fn invalidate_provider(",helper+"pub(crate) fn invalidate_provider(")
    f.write_text(data)
    f=Path("src-tauri/src/lib.rs")
    data=f.read_text()
    data=checked_replace(data,"    Ok(catalog)\n","    slash_commands::filter_disabled(&provider, catalog, &cwd).await\n")
    f.write_text(data)

def adapt_tests():
    for name in ("playwright.config.ts","playwright.webkit.config.ts"):
        f=Path(name)
        data=f.read_text()
        if "'**/skill-manager.spec.ts'" not in data:
            data=checked_replace(data,"'**/skills-library.spec.ts',","'**/skills-library.spec.ts', '**/skill-manager.spec.ts',")
        if name=="playwright.webkit.config.ts" and "|Skill manager|" not in data:
            data=checked_replace(data,"|Skills & rules|","|Rules & library|Skill manager|")
        f.write_text(data)

def docs():
    f=Path("docs/skills-library.md")
    f.write_text(source_file(str(f))+"\n## PandaCode Extended Customize Skills\n\nThe separate [Customize → Skills workspace](customize-skills.md) retains ZIP imports, GitHub sources, multiselect, per-harness installation, replacement, and sync progress. The upstream Skills Manager governs runtime enabled/disabled status.\n")
    f=Path("website/src/data/release-dates.json")
    old=json.loads(git("show",TARGET+":"+str(f)))
    new=json.loads(git("show",SOURCE+":"+str(f)))
    if not isinstance(old,dict) or not isinstance(new,dict): raise RuntimeError("Unexpected release dates schema")
    old.update(new)
    f.write_text(json.dumps(old,indent=2,ensure_ascii=False)+"\n")
    f=Path("docs/extended/0.12.0/base-reconciliation-2026-11-10.md")
    f.parent.mkdir(parents=True,exist_ok=True)
    f.write_text(git("show","origin/"+BRANCH+":"+str(f))+"\n")

def tree_map(rev):
    raw=subprocess.check_output(["git","ls-tree","-rz",rev])
    result={}
    for item in raw.split(b"\x00"):
        if item:
            head,path=item.split(b"\t",1)
            mode,kind,sha=head.decode().split()
            result[os.fsdecode(path)]=mode+":"+sha
    return result

def verify_inventory(candidate):
    b,s,t,c=map(tree_map,(PREVIOUS,SOURCE,TARGET,candidate))
    counts={"source_only":0,"target_only":0,"both_same":0,"both_changed":0,"unchanged":0}
    exceptions={"src/skills-library.ts","docs/skills-library.md","website/src/data/release-dates.json",
                "docs/extended/0.12.0/base-reconciliation-2026-11-10.md"}
    for p in set(b)|set(s)|set(t):
        base,source,target=b.get(p),s.get(p),t.get(p)
        group=("unchanged" if base==source else "both_same") if source==target else ("source_only" if base==target else ("target_only" if base==source else "both_changed"))
        counts[group]+=1
        if p not in exceptions and ((group=="source_only" and c.get(p)!=source) or
                                   (group=="target_only" and c.get(p)!=target) or
                                   (group=="both_same" and c.get(p)!=source)):
            raise RuntimeError("Lost "+group+" content "+p)
    must=("src-tauri/src/skill_manager.rs","src-tauri/src/skill_manager/launch.rs",
          "src-tauri/src/skill_manager/matrix.rs","src/skill-manager.ts",
          "src/preview-skill-manager.ts","tests/skill-manager.spec.ts",
          "src/customize-skills.ts","src-tauri/src/architect/reviewer_security.rs",
          "src-tauri/src/skills/sources.rs")
    for p in must:
        if p not in c: raise RuntimeError("Missing feature "+p)
    for p in ("src-tauri/src/skills/catalog.rs","src-tauri/src/skills/catalog_tests.rs"):
        if p in c: raise RuntimeError("Obsolete source catalog still present "+p)
    print("INVENTORY",json.dumps(counts,sort_keys=True))

def main():
    git("config","user.name","PandaCode Reconciliation")
    git("config","user.email","reconciliation@users.noreply.github.com")
    if git("rev-parse","origin/"+BRANCH)!=os.environ["EXPECTED_HEAD"]:
        raise RuntimeError("Topic branch changed")
    if git("merge-base",SOURCE,TARGET)!="dbd4e3f39685603bff21069439edf5e5201e021d":
        raise RuntimeError("Original source/destination merge base drifted")
    root=git("commit-tree",git("rev-parse",PREVIOUS+"^{tree}"),"-m","virtual prior source")
    newer=git("commit-tree",git("rev-parse",SOURCE+"^{tree}"),"-p",root,"-m","virtual upstream source")
    dest=git("commit-tree",git("rev-parse",TARGET+"^{tree}"),"-p",root,"-m","virtual Extended destination")
    git("switch","-c","reconciliation-test",dest)
    merged=subprocess.run(["git","merge","--no-commit","--no-ff",newer],capture_output=True,text=True)
    if merged.returncode not in (0,1): raise RuntimeError(merged.stderr)
    conflicts=git("diff","--name-only","--diff-filter=U").splitlines()
    expect={"docs/extended/0.12.0/README.md","docs/extended/0.12.0/dependency-security.md",
      "docs/extended/0.12.0/feature-controls.md","docs/extended/0.12.0/known-issues.md",
      "docs/extended/0.12.0/release-checklist.md","docs/extended/README.md",
      "docs/extended/upstream/source-baseline.json","docs/skills-library.md",
      "package-lock.json","package.json","playwright.config.ts","playwright.webkit.config.ts",
      "releases/extended/0.12.0.md","src-tauri/Cargo.lock","src-tauri/Cargo.toml",
      "src-tauri/src/agent_tools.rs","src-tauri/src/architect.rs","src-tauri/src/bots.rs",
      "src-tauri/src/lib.rs","src-tauri/src/providers.rs","src-tauri/src/skills.rs",
      "src-tauri/src/skills/catalog.rs","src-tauri/src/skills/catalog_tests.rs",
      "src-tauri/tauri.conf.json","src/main.ts","src/preview-skills.ts",
      "src/preview.ts","src/skills-library.ts","website/src/data/release-dates.json"}
    if set(conflicts)!=expect:
        raise RuntimeError("Conflict inventory changed: missing "+str(sorted(expect-set(conflicts)))+" extra "+str(sorted(set(conflicts)-expect)))
    special={"src/skills-library.ts","docs/skills-library.md","website/src/data/release-dates.json"}
    drop={"src-tauri/src/skills/catalog.rs","src-tauri/src/skills/catalog_tests.rs"}
    for p in conflicts:
        if p in drop:
            git("rm","--",p)
            print("RETIRED",p)
        elif p not in special:
            resolve_conflict_file(p)
    adapt_rules_library()
    adapt_preview()
    adapt_rust()
    adapt_tests()
    docs()
    git("add","-A")
    if git("diff","--name-only","--diff-filter=U"): raise RuntimeError("Unmerged paths remain")
    tree=git("write-tree")
    verify_inventory(tree)
    if subprocess.run(["git","diff","--cached","--check"],capture_output=True).returncode:
        raise RuntimeError("Whitespace check failed")
    Path(os.environ.get("GITHUB_ENV","/tmp/reconcile.env")).open("a").write("CANDIDATE_TREE="+tree+"\n")
    print("RECONCILIATION_CANDIDATE_READY",tree)

if __name__=="__main__":
    try: main()
    except Exception as error:
        print("RECONCILIATION_BLOCKED",repr(error),file=sys.stderr)
        raise
