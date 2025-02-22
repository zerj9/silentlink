"use client";

import { useState } from "react";

export default function Login() {
  const [loading, setLoading] = useState(false);

  const handleGoogleSignIn = async () => {
    setLoading(true);
    try {
      const response = await fetch("http://localhost:3210/auth/url", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        throw new Error("Failed to get the authentication URL");
      }

      const data = await response.json();
      const authUrl = data.url; // assuming the API returns { url: "..." }
      console.log("Redirecting to:", authUrl);
      window.location.href = authUrl;
    } catch (error) {
      console.error("Error during sign-in:", error);
      // Optionally, add error handling UI here
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="relative min-h-screen overflow-hidden bg-gray-900">
      {/* Background Gradient Overlay */}
      <div className="absolute inset-0 bg-gradient-to-r from-blue-500 to-green-500 opacity-10"></div>

      {/* Centered Login Content */}
      <div className="relative z-10 flex flex-col items-center justify-center min-h-screen px-6">
        <h1 className="text-center text-5xl md:text-7xl font-extrabold mb-6 pb-2 bg-clip-text text-transparent bg-gradient-to-r from-blue-400 to-green-400">
          Sign In to SilentLink
        </h1>
        <p className="max-w-md text-xl md:text-2xl text-gray-300 mb-8 text-center">
          Sign in to access your account and start exploring the power of a connected knowledge base.
        </p>
        <button
          onClick={handleGoogleSignIn}
          className="flex items-center gap-4 px-8 py-4 bg-white text-gray-800 hover:bg-gray-200 font-semibold rounded-full shadow-lg transition-colors"
          disabled={loading}
        >
          <GoogleIcon />
          <span>{loading ? "Loading..." : "Sign in with Google"}</span>
        </button>
      </div>
    </div>
  );
}

function GoogleIcon() {
  return (
    <svg className="w-6 h-6" viewBox="0 0 48 48">
      <path
        fill="#EA4335"
        d="M24 9.5c3.9 0 6.8 1.7 8.4 3.1l5.9-5.9C34.1 3.8 29.4 2 24 2 14.9 2 6.8 7.4 2.7 14.3l7 5.5C11.9 14.1 17.4 9.5 24 9.5z"
      />
      <path
        fill="#4285F4"
        d="M46.1 24.5c0-1.5-.1-2.4-.3-3.5H24v6.6h12.7c-.5 3.1-2.2 6-4.8 7.8l7.4 5.7C44.7 36.1 46.1 30.9 46.1 24.5z"
      />
      <path
        fill="#FBBC05"
        d="M10.7 28.4c-1.2-3.1-1.2-6.5 0-9.6l-7-5.5C.9 18.7 0 21.3 0 24s.9 5.3 3.7 9l7-5.6z"
      />
      <path
        fill="#34A853"
        d="M24 46c6.5 0 12-2.2 16-6l-7.4-5.7c-2.1 1.4-4.8 2.3-8 2.3-6.5 0-12-4.4-14-10.3l-7 5.5C6.8 40.6 14.9 46 24 46z"
      />
      <path fill="none" d="M0 0h48v48H0z" />
    </svg>
  );
}
