'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import React from 'react';

const organisations = [
  {
    id: 'org1',
    name: 'Public',
    workspaces: [
      {
        id: 'ws1',
        name: 'National',
        projects: [
          { id: '1', name: 'UK Risk Register', href: '/projects/alpha' },
          { id: '2', name: 'Beta', href: '/projects/beta' },
        ],
      },
      {
        id: 'ws2',
        name: 'US - National',
        projects: [
          { id: '3', name: 'Gamma', href: '/projects/gamma' },
          { id: '4', name: 'Delta', href: '/projects/delta' },
        ],
      },
    ],
  },
  {
    id: 'org2',
    name: 'Organisation B',
    workspaces: [
      {
        id: 'ws3',
        name: 'Workspace 3',
        projects: [
          { id: '5', name: 'Epsilon', href: '/projects/epsilon' },
          { id: '6', name: 'Zeta', href: '/projects/zeta' },
        ],
      },
    ],
  },
];

export default function Dashboard() {
  // Track selected organisation.
  const [selectedOrgId, setSelectedOrgId] = useState(organisations[0].id);
  const selectedOrg = organisations.find((org) => org.id === selectedOrgId);

  // Track selected workspace.
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState(
    selectedOrg.workspaces[0]?.id || null
  );

  // When the selected organisation changes, reset the workspace selection.
  useEffect(() => {
    if (selectedOrg && selectedOrg.workspaces.length > 0) {
      setSelectedWorkspaceId(selectedOrg.workspaces[0].id);
    } else {
      setSelectedWorkspaceId(null);
    }
  }, [selectedOrg]);

  // Find the selected workspace.
  const selectedWorkspace = selectedOrg.workspaces.find(
    (ws) => ws.id === selectedWorkspaceId
  );

  return (
    <div className="min-h-screen flex bg-gray-50">
      {/* Sidebar */}
      <aside className="w-64 bg-white border-r border-gray-200 shadow-sm flex flex-col justify-between">
        <div className="p-6">
          {/* Organisations Dropdown */}
          <div className="mb-8">
            <label
              htmlFor="organisation"
              className="block text-xl font-bold text-gray-800 mb-2"
            >
              Organisation
            </label>
            <div className="relative">
              <select
                id="organisation"
                value={selectedOrgId}
                onChange={(e) => setSelectedOrgId(e.target.value)}
                className="w-full px-4 py-2 rounded-md border border-gray-300 shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-400 appearance-none bg-white"
              >
                {organisations.map((org) => (
                  <option key={org.id} value={org.id}>
                    {org.name}
                  </option>
                ))}
              </select>
              <div className="pointer-events-none absolute inset-y-0 right-0 flex items-center px-3 text-gray-700">
                <svg
                  className="fill-current h-4 w-4"
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 20 20"
                >
                  <path d="M5.516 7.548l4.484 4.482 4.484-4.482L16 8.548l-6 6-6-6z" />
                </svg>
              </div>
            </div>
          </div>

          {/* Workspace List */}
          {selectedOrg && (
            <div>
              <h2 className="text-xl font-bold text-gray-800 mb-4">Workspaces</h2>
              <nav className="flex flex-col space-y-2">
                {selectedOrg.workspaces.map((ws) => (
                  <button
                    key={ws.id}
                    onClick={() => setSelectedWorkspaceId(ws.id)}
                    className={`text-left px-4 py-3 rounded-md transition-colors focus:outline-none ${
                      selectedWorkspaceId === ws.id
                        ? 'bg-green-100 text-green-700 font-medium'
                        : 'text-gray-700 hover:bg-gray-100'
                    }`}
                  >
                    {ws.name}
                  </button>
                ))}
              </nav>
            </div>
          )}
        </div>

        {/* Sign Out Button */}
        <div className="p-6 border-t border-gray-200">
          <button
            onClick={() => {
              // Add your sign out logic here.
              console.log('Signing out...');
            }}
            className="w-full px-4 py-2 bg-red-700 text-white rounded-md hover:bg-red-600 transition-colors"
          >
            Sign Out
          </button>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 p-8">
        <header className="mb-10">
          {selectedWorkspace && (
            <React.Fragment>
            <h1 className="text-2xl font-semibold text-gray-700 mt-2">
              Projects
            </h1>
            <h2>Selected Workspace: {selectedWorkspace.name}</h2>
            </React.Fragment>
          )}
        </header>

        {selectedWorkspace ? (
          <section>
            <div className="grid gap-6 grid-cols-1 md:grid-cols-2 lg:grid-cols-3">
              {selectedWorkspace.projects.map((project) => (
                <div
                  key={project.id}
                  className="bg-white rounded-lg shadow hover:shadow-lg transition-shadow p-6"
                >
                  <Link
                    href={project.href}
                    className="block bg-blue-500 text-white text-center py-2 rounded-md hover:bg-blue-600 transition-colors"
                  >
                    {project.name}
                  </Link>
                </div>
              ))}
            </div>
          </section>
        ) : (
          <p className="text-gray-600">No workspace selected.</p>
        )}
      </main>
    </div>
  );
}
