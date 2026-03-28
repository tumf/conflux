import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { Folder, FolderOpen, File, ChevronRight, ChevronDown } from 'lucide-react';
import { FileBrowseContext, FileTreeEntry, FileContentResponse } from '../api/types';
import { fetchFileTree, fetchFileContent } from '../api/restClient';

interface FileViewPanelProps {
  projectId: string | null;
  context: FileBrowseContext | null;
}

interface TreeNodeProps {
  entry: FileTreeEntry;
  expandedPaths: Set<string>;
  selectedFilePath: string | null;
  onToggleDir: (path: string) => void;
  onSelectFile: (path: string) => void;
  depth: number;
}

function TreeNode({ entry, expandedPaths, selectedFilePath, onToggleDir, onSelectFile, depth }: TreeNodeProps) {
  const isDir = entry.type === 'directory';
  const isExpanded = expandedPaths.has(entry.path);
  const isSelected = selectedFilePath === entry.path;

  const handleClick = useCallback(() => {
    if (isDir) {
      onToggleDir(entry.path);
    } else {
      onSelectFile(entry.path);
    }
  }, [isDir, entry.path, onToggleDir, onSelectFile]);

  return (
    <>
      <button
        onClick={handleClick}
        className={`flex w-full items-center gap-1 py-0.5 text-left text-xs transition-colors hover:bg-[#27272a]/50 ${
          isSelected ? 'bg-[#1e1b4b]/50 text-[#a5b4fc]' : 'text-[#a1a1aa]'
        }`}
        style={{ paddingLeft: `${depth * 12 + 4}px` }}
      >
        {isDir ? (
          <>
            {isExpanded ? (
              <ChevronDown className="size-3 shrink-0 text-[#52525b]" />
            ) : (
              <ChevronRight className="size-3 shrink-0 text-[#52525b]" />
            )}
            {isExpanded ? (
              <FolderOpen className="size-3.5 shrink-0 text-[#6366f1]" />
            ) : (
              <Folder className="size-3.5 shrink-0 text-[#6366f1]" />
            )}
          </>
        ) : (
          <>
            <span className="size-3 shrink-0" />
            <File className="size-3.5 shrink-0 text-[#52525b]" />
          </>
        )}
        <span className="truncate">{entry.name}</span>
      </button>
      {isDir && isExpanded && entry.children && (
        <>
          {entry.children.map((child) => (
            <TreeNode
              key={child.path}
              entry={child}
              expandedPaths={expandedPaths}
              selectedFilePath={selectedFilePath}
              onToggleDir={onToggleDir}
              onSelectFile={onSelectFile}
              depth={depth + 1}
            />
          ))}
        </>
      )}
    </>
  );
}

/** Collect all ancestor directory paths for a given file path */
function getAncestorPaths(filePath: string): string[] {
  const parts = filePath.split('/');
  const paths: string[] = [];
  for (let i = 1; i < parts.length; i++) {
    paths.push(parts.slice(0, i).join('/'));
  }
  return paths;
}

/** Find the first file at a given directory path in the tree */
function findFileInTree(tree: FileTreeEntry[], dirPath: string, fileName: string): string | null {
  for (const entry of tree) {
    if (entry.type === 'file' && entry.path === `${dirPath}/${fileName}`) {
      return entry.path;
    }
    if (entry.type === 'directory' && entry.children) {
      const found = findFileInTree(entry.children, dirPath, fileName);
      if (found) return found;
    }
  }
  return null;
}

export function FileViewPanel({ projectId, context }: FileViewPanelProps) {
  const [tree, setTree] = useState<FileTreeEntry[]>([]);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<FileContentResponse | null>(null);
  const [isLoadingTree, setIsLoadingTree] = useState(false);
  const [isLoadingContent, setIsLoadingContent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Determine root parameter
  const rootParam = useMemo(() => {
    if (!context) return 'base';
    if (context.type === 'change') return 'base';
    if (context.type === 'worktree' && context.worktreeBranch) {
      return `worktree:${context.worktreeBranch}`;
    }
    return 'base';
  }, [context]);

  // Load file tree when project or context changes
  useEffect(() => {
    if (!projectId || !context) {
      setTree([]);
      setSelectedFilePath(null);
      setFileContent(null);
      return;
    }

    let cancelled = false;
    setIsLoadingTree(true);
    setError(null);
    setSelectedFilePath(null);
    setFileContent(null);

    fetchFileTree(projectId, rootParam)
      .then((entries) => {
        if (cancelled) return;
        setTree(entries);
        setIsLoadingTree(false);

        // Auto-expand for change context
        if (context.type === 'change' && context.changeId) {
          const changeDirPath = `openspec/changes/${context.changeId}`;
          const ancestors = getAncestorPaths(changeDirPath + '/proposal.md');
          // Include the change directory itself
          ancestors.push(changeDirPath);
          setExpandedPaths(new Set(ancestors));

          // Auto-select proposal.md
          const proposalPath = `${changeDirPath}/proposal.md`;
          const found = findFileInTree(entries, changeDirPath, 'proposal.md');
          if (found) {
            setSelectedFilePath(proposalPath);
          }
        } else {
          setExpandedPaths(new Set());
        }
      })
      .catch((err) => {
        if (cancelled) return;
        setIsLoadingTree(false);
        setError(err instanceof Error ? err.message : String(err));
      });

    return () => {
      cancelled = true;
    };
  }, [projectId, context, rootParam]);

  // Load file content when selected file changes
  useEffect(() => {
    if (!projectId || !selectedFilePath) {
      setFileContent(null);
      return;
    }

    let cancelled = false;
    setIsLoadingContent(true);

    fetchFileContent(projectId, rootParam, selectedFilePath)
      .then((response) => {
        if (cancelled) return;
        setFileContent(response);
        setIsLoadingContent(false);
      })
      .catch((err) => {
        if (cancelled) return;
        setIsLoadingContent(false);
        setFileContent(null);
        setError(err instanceof Error ? err.message : String(err));
      });

    return () => {
      cancelled = true;
    };
  }, [projectId, rootParam, selectedFilePath]);

  const handleToggleDir = useCallback((path: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  const handleSelectFile = useCallback((path: string) => {
    setSelectedFilePath(path);
  }, []);

  // Placeholder when no context
  if (!context) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">Select a change or worktree to browse files</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#ef4444]">{error}</p>
      </div>
    );
  }

  if (isLoadingTree) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">Loading files...</p>
      </div>
    );
  }

  return (
    <div className="flex flex-1 overflow-hidden">
      {/* File tree (left) */}
      <div className="w-48 shrink-0 overflow-y-auto border-r border-[#27272a] py-1">
        {tree.length === 0 ? (
          <div className="flex items-center justify-center p-4">
            <p className="text-xs text-[#52525b]">No files</p>
          </div>
        ) : (
          tree.map((entry) => (
            <TreeNode
              key={entry.path}
              entry={entry}
              expandedPaths={expandedPaths}
              selectedFilePath={selectedFilePath}
              onToggleDir={handleToggleDir}
              onSelectFile={handleSelectFile}
              depth={0}
            />
          ))
        )}
      </div>

      {/* File content (right) */}
      <div className="flex flex-1 flex-col overflow-hidden">
        {!selectedFilePath && (
          <div className="flex flex-1 items-center justify-center p-8">
            <p className="text-sm text-[#52525b]">Select a file to view its content</p>
          </div>
        )}

        {selectedFilePath && isLoadingContent && (
          <div className="flex flex-1 items-center justify-center p-8">
            <p className="text-sm text-[#52525b]">Loading...</p>
          </div>
        )}

        {selectedFilePath && !isLoadingContent && fileContent && (
          <>
            <div className="flex items-center justify-between border-b border-[#27272a] px-3 py-1.5">
              <span className="truncate font-mono text-xs text-[#71717a]">{fileContent.path}</span>
              <div className="flex items-center gap-2 text-xs text-[#52525b]">
                {fileContent.truncated && (
                  <span className="text-[#f59e0b]">truncated</span>
                )}
                <span>{formatFileSize(fileContent.size)}</span>
              </div>
            </div>
            <div className="flex-1 overflow-auto">
              {fileContent.binary ? (
                <div className="flex flex-1 items-center justify-center p-8">
                  <p className="text-sm text-[#52525b]">Binary file - cannot display</p>
                </div>
              ) : fileContent.content !== null ? (
                <pre className="p-3 font-mono text-xs leading-relaxed text-[#d4d4d8]">
                  <code>
                    {fileContent.content.split('\n').map((line, idx) => (
                      <div key={idx} className="flex">
                        <span className="mr-3 inline-block w-8 shrink-0 select-none text-right text-[#3f3f46]">
                          {idx + 1}
                        </span>
                        <span className="whitespace-pre-wrap break-all">{line}</span>
                      </div>
                    ))}
                  </code>
                </pre>
              ) : null}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
