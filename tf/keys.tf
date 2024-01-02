resource "aws_key_pair" "deep-freeze-key-pair" {
  key_name   = "tf-key-pair"
  public_key = tls_private_key.rsa.public_key_openssh

  provisioner "local-exec" {
    command = "echo ${tls_private_key.rsa.private_key_pem} > ./tf-key-pair.pem"
  }
}

resource "tls_private_key" "rsa" {
  algorithm = "RSA"
  rsa_bits  = 4096
}

resource "local_file" "deep-freeze-private-key" {
  content  = tls_private_key.rsa.private_key_pem
  filename = "tf-key-pair"
}
