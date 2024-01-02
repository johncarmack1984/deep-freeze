resource "aws_instance" "deep-freeze" {
  ami                         = var.ami_id
  instance_type               = var.aws_config.instance_type
  key_name                    = aws_key_pair.deep-freeze-key-pair.key_name
  security_groups             = ["${aws_security_group.ingress-all-test.id}"]
  vpc_security_group_ids      = ["${aws_security_group.ingress-all-test.id}"]
  associate_public_ip_address = true
  iam_instance_profile        = aws_iam_instance_profile.s3_instance_profile.name
  subnet_id                   = aws_subnet.deep-freeze-subnet-1.id

  root_block_device {
    volume_size = var.aws_config.volume_size
  }

  credit_specification {
    cpu_credits = "unlimited"
  }

  connection {
    type        = "ssh"
    user        = "ubuntu"
    private_key = file(local_file.deep-freeze-private-key.filename)
    host        = self.public_ip
  }

  provisioner "file" {
    source      = ".env"
    destination = "/home/ubuntu/.env"
  }
  provisioner "file" {
    source      = "prepare1.sh"
    destination = "/home/ubuntu/prepare1.sh"
  }
  provisioner "file" {
    source      = "prepare2.sh"
    destination = "/home/ubuntu/prepare2.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /home/ubuntu/prepare1.sh",
      "chmod +x /home/ubuntu/prepare2.sh",
    ]
  }
  tags = {
    Name = "ami-0f8e81a3da6e2510a"
  }
}
