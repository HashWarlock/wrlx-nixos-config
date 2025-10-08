{
  ##################################################################################################################
  #
  # Confidant User - NixOS Configuration for CVM
  #
  ##################################################################################################################

  users.users.confidant = {
    isNormalUser = true;
    description = "Confidant CVM User";
    extraGroups = ["wheel" "docker" "networkmanager"];

    # Optional: Set a hashed password for emergency console access
    # Generate with: mkpasswd -m sha-512
    # hashedPassword = "$6$...";

    # SSH key authentication (replace with your actual public key)
    openssh.authorizedKeys.keys = [
      # Add your SSH public key here
      # Example: "ssh-ed25519 AAAAC3Nza... confidant@phala-cvm"
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFCWKK5NaezU9UP6dug+pDfiVeglHBjzwyHwz6In/AgJ hashwarlock@phala.network"
    ];
  };
}
