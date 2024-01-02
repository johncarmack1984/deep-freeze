resource "aws_subnet" "deep-freeze-subnet-1" {
  cidr_block        = cidrsubnet(aws_vpc.deep-freeze-vpc.cidr_block, 3, 1)
  vpc_id            = aws_vpc.deep-freeze-vpc.id
  availability_zone = "${var.aws_config.region}a"
}

resource "aws_route_table" "deep-freeze-route-table" {
  vpc_id = aws_vpc.deep-freeze-vpc.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.deep-freeze-igw.id
  }

  tags = {
    Name = "deep-freeze-route-table"
  }
}

resource "aws_route_table_association" "subnet-association" {
  subnet_id      = aws_subnet.deep-freeze-subnet-1.id
  route_table_id = aws_route_table.deep-freeze-route-table.id
}
