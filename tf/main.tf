terraform {
  required_version = ">= 1.5.2"

  backend "s3" {
    bucket = "deep-freeze-backend"
    key    = "terraform.tfstate"
    region = "us-east-1"
  }

  required_providers {

    aws = {
      source  = "hashicorp/aws"
      version = "5.3.0"
    }

    random = {
      source  = "hashicorp/random"
      version = "3.5.1"
    }
  }
}
