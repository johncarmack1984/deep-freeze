output "instance_id" {
  description = "ID of the EC2 instance"
  value       = aws_instance.deep-freeze.id
}

output "instance_public_ip" {
  description = "Public IP address of the EC2 instance"
  value       = aws_instance.deep-freeze.public_ip
}

output "instance_public_dns" {
  description = "Public IP address of the EC2 instance"
  value       = aws_instance.deep-freeze.public_dns
}

output "ssh_command" {
  description = "Command to use to SSH to the instance"
  value       = "ssh -i <key> ubuntu@${aws_instance.deep-freeze.public_dns}"
}

output "web-address" {
  value = "${aws_instance.deep-freeze.public_dns}:8080"
}
