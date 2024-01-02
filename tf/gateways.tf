resource "aws_internet_gateway" "deep-freeze-igw" {
  vpc_id = aws_vpc.deep-freeze-vpc.id

  tags = {
    Name = "deep-freeze-igw"
  }
}
