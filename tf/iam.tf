resource "aws_iam_role" "s3_role" {
  name = "S3FullAccessRole"

  assume_role_policy = <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Action": "sts:AssumeRole",
      "Principal": {
        "Service": "ec2.amazonaws.com"
      },
      "Effect": "Allow",
      "Sid": ""
    }
  ]
}
EOF
}

resource "aws_iam_role_policy" "s3_policy" {
  name = "S3FullAccessPolicy"
  role = aws_iam_role.s3_role.id

  policy = <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "s3:*"
      ],
      "Resource": [
        "arn:aws:s3:::${var.aws_config.bucket}/*"
      ]
    }
  ]
}
EOF
}

resource "aws_iam_instance_profile" "s3_instance_profile" {
  name = "S3InstanceProfile"
  role = aws_iam_role.s3_role.name
}
