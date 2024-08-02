{
	description = "Command-line file backup tool";
	
	inputs = {
		nixpkgs = {
			url = "github:nixos/nixpkgs/nixpkgs-unstable";
		};
		
		flake-utils = {
			url = "github:numtide/flake-utils";
		};
		
		crane = {
			url = "github:ipetkov/crane";
			inputs.nixpkgs.follows = "nixpkgs";
		};
		
		fenix = {
			url = "github:nix-community/fenix";
			inputs.nixpkgs.follows = "nixpkgs";
		};
	};
	
	outputs = { self, nixpkgs, flake-utils, ... } @ inputs: let
		makeCraneLib = pkgs: let
			fenixToolchain = pkgs.fenix.stable.defaultToolchain;
		in (inputs.crane.mkLib pkgs).overrideToolchain fenixToolchain;
	in {
		overlays = {
			fenix = final: prev: {
				fenix = inputs.fenix.packages.${prev.system};
			};
			
			backy = final: prev: {
				backy = prev.callPackage ./backy.nix {
					craneLib = makeCraneLib final;
				};
			};
			
			default = nixpkgs.lib.composeManyExtensions (with self.overlays; [
				fenix
				backy
			]);
		};
	} // flake-utils.lib.eachDefaultSystem (system:
			let
				pkgs = import nixpkgs {
					inherit system;
					overlays = [self.overlays.default];
				};
				craneLib = makeCraneLib pkgs;
			in {
				packages = {
					default = pkgs.backy;
				};
				
				checks = pkgs.backy.tests;
				
				devShells.default = craneLib.devShell {
					packages = with pkgs; [
						rust-analyzer
					];
				};
			}
		);
}
